# Changesets

Every publishable Origin SDK change must include a changeset naming the affected packages and the
pre-1.0 semver impact. Generated descriptor refreshes require a changeset only when they alter a
published package. Release commits are created with `npm run version-packages`; publishing remains a
separately authorized operation.
