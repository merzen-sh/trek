dev:
    @cargo run -p trek --features swagger

fmt:
    @cargo fmt
    @pnpm oxfmt .

[working-directory("./packages/ui")]
shadcn comment:
    @pnpm run shadcn add {{ comment }} --yes

check:
    @cargo check --workspace