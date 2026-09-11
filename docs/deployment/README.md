# Deployment profiles

Profiles select local policy values and physical topology. They do not change the MMP/1 wire grammar. The profile documents below describe evidence targets, not delivery guarantees.

- [`profiles.md`](profiles.md): profile format and tier mapping.
- [`train.md`](train.md): infrastructure-assisted and ad-hoc train deployments.
- [`campus.md`](campus.md): building and campus deployment.

Authority certificates are optional in every profile. A profile MUST identify which values are interoperability constants, local safety bounds, or research parameters. A profile MUST NOT claim a battery or scale result until the corresponding measurement exists.
