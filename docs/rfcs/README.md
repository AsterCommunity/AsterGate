# AsterGate RFCs

This directory is the design authority for AsterGate product architecture. RFCs describe target
contracts and acceptance criteria; they do not claim that the current skeleton already implements
them.

## Status vocabulary

- **Proposed**: under review; implementation must not silently invent conflicting behavior.
- **Accepted**: approved as the implementation contract.
- **Implemented**: the accepted behavior and its required verification are present in the current
  checkout.
- **Superseded**: replaced by another RFC, which must be linked from the old document.

Changing an accepted security boundary, public protocol, or durable data identity requires a new
RFC or an explicit amendment with migration and compatibility analysis. Smaller implementation
details may be settled in code and tests as long as they preserve the accepted contracts.

## Index

| RFC | Status | Subject |
| --- | --- | --- |
| [0001](0001-system-architecture.md) | Proposed | Product scope and system architecture |
| [0002](0002-identity-tenancy-authorization.md) | Proposed | Global Account, tenant Membership, groups, and application admission |
| [0003](0003-oauth-oidc-security.md) | Proposed | OAuth 2.1 / OpenID Connect protocol and security contract |
| [0004](0004-upstream-federation-and-asterdrive.md) | Proposed | Upstream identity federation and AsterDrive migration |
| [0005](0005-delivery-roadmap.md) | Proposed | Final product contract, delivery order, and acceptance gates |
| [0006](0006-bidirectional-connectors.md) | Proposed | Bidirectional upstream/downstream connector architecture |
| [0007](0007-instance-identity-boundaries.md) | Proposed | One identity domain per instance, independent deployments, and federation boundaries |
| [0008](0008-independent-idp-and-application-access-policy.md) | Proposed | Independent IdP product boundary and Application access policy |

## Reading order

Read RFC 0001 first. RFCs 0002 and 0003 define the durable data and protocol boundaries. RFC 0004
defines how the existing AsterDrive external-login implementation is reused without making
AsterGate depend on AsterDrive product semantics. RFC 0005 specifies the final capability set and
the evidence for dependency-ordered vertical delivery, without temporary phase architectures.
RFC 0006 defines the two connector directions, shared catalog, protocol-family ports, and one
identity/authorization/issuance core.
RFC 0007 amends the instance/identity-domain terminology and deployment boundaries in RFCs
0001, 0002, 0003, 0005, and 0006. It replaces the proposed singleton Realm entity with instance
configuration and uses independent deployments for independent identity domains.
RFC 0008 defines AsterGate as an independent general-purpose IdP and freezes the distinction
between Group-based Application admission and Role/Permission-based resource authorization.

## Evidence and references

The initial RFC set was derived from the current AsterGate skeleton, the current AsterDrive
external-auth implementation and design documents, the `aster_forge_external_auth` boundary, and
the public Logto documentation. Normative OAuth, OpenID Connect, JOSE, WebAuthn, and security
behavior is governed by the referenced standards rather than by competitor behavior.
