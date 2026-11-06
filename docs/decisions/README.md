# Architecture decisions

Keep significant decisions next to the code so contributors can understand the
accepted behavior and its tradeoffs. Existing economics and custody decisions
are recorded in [ARCHITECTURE.md](../../ARCHITECTURE.md); this directory does not
create an additional approval body.

Use a sequential filename such as `0001-subject.md` for a new proposal. A decision
records the following fields:

| Field | Required content |
|---|---|
| Title and status | Proposed, accepted, superseded or rejected; decision date |
| Context | Concrete problem, affected users/components and constraints |
| Decision | Final behavior and the authority required to exercise it |
| Alternatives | Credible options and the reason for the chosen tradeoff |
| Consequences | Economics, custody, authorization, resource and operational effects |
| Compatibility | ABI/config/storage/domain/events, client versions and immutable releases |
| Validation | Reproduction, acceptance cases and evidence still required |
| Maintenance | Owner role, failure recovery, dependency and retirement effects |
| References | Issue/pull request, related decisions and authoritative specifications |

Record who made the decision using actual repository identities when available.
Do not fabricate a review roster. Preserve superseded decisions and link their
replacements; current architecture and API references must reflect the final
accepted state.

