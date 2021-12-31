#[cfg(target_os = "linux")]
fn main() -> anyhow::Result<()> {
    use anyhow::Context;
    use harness::{MachineExt, MultiaddrExt, NetsimExt};
    use ipfs_embed_cli::{Command, Config, Event};
    use maplit::hashmap;
    use netsim_embed::{Ipv4Range, NatConfig};

    harness::build_bin()?;

    harness::run_netsim(|mut sim, opts, net_a, temp_dir| async move {
        let range_b = {
            let range_a = sim.network(net_a).range();
            loop {
                let r = Ipv4Range::random_local_subnet();
                if r != range_a {
                    break r;
                }
            }
        };
        let net_b = sim.spawn_network(range_b);
        let nat = NatConfig::default();
        sim.add_nat_route(nat, net_a, net_b);

        let consumers = opts.n_nodes..2 * opts.n_nodes;
        for i in consumers.clone() {
            let cfg = Config {
                path: Some(temp_dir.path().join(i.to_string())),
                node_name: Some(format!("consumer-{}", i)),
                keypair: i as _,
                listen_on: vec!["/ip4/0.0.0.0/tcp/30000".parse().unwrap()],
                bootstrap: vec![],
                external: vec![],
                enable_mdns: opts.enable_mdns,
                disable_port_reuse: opts.disable_port_reuse,
            };
            let cmd = async_process::Command::from(cfg);
            let machine = sim.spawn_machine(cmd, None).await;
            sim.plug(machine, net_b, None).await;
            let m = sim.machine(machine);
            tracing::warn!(
                "{} started with address {} and peer id {}",
                machine,
                m.addr(),
                m.peer_id(),
            );
        }

        let providers = sim.nodes(0..opts.n_nodes);
        let consumers = sim.nodes(consumers);

        for id in providers.keys().chain(consumers.keys()) {
            let m = sim.machine(*id);
            m.select(|e| matches!(e, Event::NewListenAddr(a) if !a.is_loopback()).then(|| ()))
                .await;
        }

        for id in consumers.keys() {
            let m = sim.machine(*id);
            for (peer, addr) in providers.values() {
                m.send(Command::AddAddress(*peer, addr.clone()));
                m.send(Command::Dial(*peer));
            }
        }

        for id in consumers.keys() {
            let m = sim.machine(*id);
            for (peer, addr) in providers.values() {
                m.select(|e| {
                    matches!(e, Event::PeerInfo(p, i)
                        if p == peer && i.addresses == hashmap!(addr.clone() => "Dial".to_owned())
                    )
                    .then(|| ())
                })
                .await;
            }
        }

        if opts.disable_port_reuse {
            for id in providers.keys() {
                let m = sim.machine(*id);
                for (peer, _addr) in consumers.values() {
                    m.select(|e| {
                        matches!(e, Event::PeerInfo(p, i) if p == peer && i.addresses.is_empty())
                            .then(|| ())
                    })
                    .await;
                }
            }
        } else {
            for id in providers.keys() {
                let m = sim.machine(*id);
                for (peer, _addr) in consumers.values() {
                    m.select(|e| {
                        matches!(e, Event::PeerInfo(p, i) if p == peer && (
                            // port_reuse unfortunately means that the NATed port is added to listeners
                            // by GenTcp, sent via Identify, but not falsifiable because we can’t attempt
                            // to dial while the connection exists
                            i.addresses == hashmap!(i.connections[0].clone() => "Candidate".to_owned())
                        ))
                        .then(|| ())
                    })
                    .await;
                }
                m.drain();
            }

            // now disconnect the consumers so that the providers will try to dial and falsify the addresses
            for id in consumers.keys() {
                sim.machine(*id).down();
            }

            for id in providers.keys() {
                let m = sim.machine(*id);
                for (peer, _addr) in consumers.values() {
                    m.select_draining(|e| {
                        matches!(e, Event::Disconnected(p) if p == peer).then(|| ())
                    })
                    .await;
                    m.send(Command::Dial(*peer));
                    m.select(|e| {
                        // prune_addresses will remove the peer when a failure happens while not connected
                        matches!(e, Event::PeerRemoved(p) if p == peer).then(|| ())
                    })
                    .await;
                }
            }
        }

        Ok(())
    })
    .context("netsim")
}

#[cfg(not(target_os = "linux"))]
fn main() {}