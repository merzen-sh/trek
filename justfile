dev:
    @cargo run -p trek

fmt:
    @cargo fmt
    @pnpm oxfmt .

[working-directory("./packages/ui")]
shadcn comment:
    @pnpm run shadcn add {{ comment }} --yes
