dev:
    @cargo run -p trek --features swagger

fmt:
    @cargo fmt
    @pnpm oxfmt .

[working-directory("./packages/ui")]
shadcn comment:
    @pnpm run shadcn add {{ comment }} --yes

[working-directory("./packages/api-types")]
api-types:
    @pnpm run generate-api-types

check:
    @cargo check --workspace

shear:
    @cargo shear --fix