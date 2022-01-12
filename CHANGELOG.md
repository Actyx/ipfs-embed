# Changelog

This changelog was begun after 0.22.

## Release 0.23 (unreleased)

- update to libp2p 0.41
- make PortReuse configurable (and recommend to turn it off)
- update to ipfs-sqlite-block-store 0.10 and thereby rusqlite 0.26
- rewrite address book logic to validate addresses and retain only confirmed ones:
  - every successful outgoing connection counts as confirmation
  - every outgoing dial failure removes unconfirmed addresses (or confirmed, if PeerId changed)
  - all discovered addresses from MDNS and Kademlia are validated by dialling
  - all incoming connections’ remote addresses are translated to likely listen addresses using IdentifyInfo
- offer detailed peer information and error history in PeerInfo
- forward all connection-related swarm events
