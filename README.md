# `engage`

A task runner with DAG-based parallelism

---

Run `cargo run` to run all groups/tasks in `engage.toml` after building a DAG
out of them.

Run with `cargo run -- --dot | dot -T svg > dag.svg && xdg-open dag.svg` to see
the output of the whole suite and also open a viewer to see the generated DAG.
