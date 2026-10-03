# RFC 0008: Independent IdP Product Boundary and Application Access Policy

- Status: Proposed
- Date: 2026-10-03
- Owners: AsterGate maintainers
- Depends on: RFC 0001, RFC 0002, RFC 0003, RFC 0006, RFC 0007

## Summary

AsterGate is an independent, self-hostable Identity Provider and Authorization Server. It is not
an AsterDrive authentication module. AsterDrive is one optional OAuth/OIDC client and resource
server; other products may integrate with the same standard contracts or run without AsterGate.
AsterForge supplies reusable runtime and infrastructure mechanics, while AsterGate owns identity,
authorization, protocol policy, migrations, and product-facing APIs.

This RFC freezes the first product-level access policy: Group membership decides whether an Account
may enter an Application, while Role and Permission decide what an admitted subject may do for a
resource. These decisions are related but never interchangeable.

## Product boundary

```text
AsterForge  -> runtime, database, migration helpers, audit mechanics, API tooling
AsterGate   -> Account, Org, Membership, Group, Application, Resource, Role, Permission, OIDC/OAuth
Any client  -> optional OIDC/OAuth integration and application-domain authorization
```

AsterGate must not contain Drive-specific workspaces, files, shares, or business permissions. A
resource server retains its own domain authorization even when it accepts a Gate-issued token.

## Application Access Policy

Every Application has one explicit admission mode:

| Mode | Decision |
| --- | --- |
| `all_active_members` | Any active Membership in the Application's Organization may enter |
| `allowed_groups` | A Membership may enter only when it belongs to at least one bound active Group |

The default is `allowed_groups` with no bindings, which denies access until an administrator binds a
Group. Multiple allowed Groups use OR semantics. There is no implicit deny-list, priority, or policy
expression in this first profile.

Admission is evaluated after Account authentication and before consent/code issuance, then again
during authorization-code exchange and refresh. It uses authoritative state and requires an active
Account, Organization, Membership, Group where applicable, and Application in the same Organization.

## Resource authorization

An Organization owns API Resources identified by stable HTTPS identifiers. Each resource declares
an allowlist of scopes. An Application may bind to a resource only from its own Organization.

For a human authorization:

```text
Membership -> active Group -> Group/Role assignment -> Role permissions -> requested Resource scope
```

A requested resource scope is issuable only when it is declared by the resource and present in the
subject's effective permissions. OIDC identity scopes remain distinct from resource scopes. Machine
clients use their explicit resource allowlist and do not impersonate a human Account.

Tokens bind issuer, audience, Organization context, subject, scope, and permissions to the grant.
A token cannot be relabeled to another resource or Organization. Resource servers still enforce
their own business rules.

## Type and administration boundaries

Management APIs expose resource, role, group-role, and Application admission operations through typed
DTOs and generated OpenAPI. Resource identifiers are immutable after registration; scope changes and
deletion check references and produce audit events. Protocol DTOs are separate from management DTOs.
Connector routes are registered by an injected descriptor registry.

Stable product concepts use typed Rust structs/enums/newtypes. `String` is allowed for user-provided
labels and opaque identifiers only when the owning boundary validates its syntax and meaning. A
repository record must not be reused as a domain command, and an unvalidated JSON object must not
become an authorization policy.

## Non-goals

- Making AsterDrive mandatory for deploying or operating AsterGate.
- Treating AsterForge internals as a public AsterGate domain model.
- Adding Realm CRUD or per-Organization issuers; RFC 0007 owns independent identity domains.
- Adding deny-list expressions, policy priorities, or arbitrary administrator code in this profile.
- Replacing application-domain authorization with Group names or a Gate permission claim.

## Acceptance gates

- A standalone AsterGate deployment can register clients, resources, roles, groups, and access
  policy without any AsterDrive tables or runtime dependency.
- A second independent client/resource server can use the same discovery, authorization, token,
  introspection, and revocation contracts.
- Positive and negative tests cover active Membership, group admission, resource scope allowlist,
  effective role permission, cross-Organization references, refresh re-evaluation, and machine
  client non-impersonation.
- OpenAPI and generated client types represent management DTOs; protocol endpoints retain normative
  OAuth/OIDC wire behavior.
- Product documentation describes AsterDrive as an optional integration.

This RFC is a product and authorization contract. It does not claim that every listed acceptance
gate is implemented in the current checkout.
