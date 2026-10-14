# Train profiles

The infrastructure-assisted profile provisions an Anchor per carriage and uses a conductor device as a physical data mule between carriages and stations. It treats the topology as a deployment fact. The ad-hoc profile has no anchors or authority chain and depends on ordinary passenger mobility. Both profiles use ordinary MMP store-carry-forward sessions.

Inter-carriage RF delivery is not assumed. A train profile that needs cross-carriage delivery MUST test the conductor-ferry path or another documented physical carrier. Platform dwell, contact duration, and RSSI values are parameters for a drill, not protocol constants. The profile should include a misdeployment drill with fewer anchors so the declared topology is tested rather than merely documented.
