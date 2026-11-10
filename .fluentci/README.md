# FluentCI workflows

GitHub installs `fluentci-io/setup-fluentci@next` and published `setup-*` modules.
The `.fluentci/ci/src` PureScript component owns CI control flow, packaging,
artifact uploads, Cachix and FlakeHub operations. Commands use literal argument
vectors. FluentCI does not install or execute Dagger; `.dagger` is retained as
legacy source.

Tooling is checked out at `github.workflow_sha` in `.ci-tooling`, independently
of the selected release source. The toolchain action pins Node 24.13.1 and
hydrates the ignored SDK before compilation. `github-env.mjs` persists setup
module environment results between processes.

Release commands perform real writes. GitHub uploads use `GH_TOKEN`; Gemfury
uses `FURY_TOKEN` and `FURY_ACCOUNT`; Cachix pushes use `CACHIX_AUTH_TOKEN`.
Missing publishing credentials skip the corresponding optional upload.
