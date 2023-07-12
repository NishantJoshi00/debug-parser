# RON Parser

Parsing debug logs with with `nom` parser. Handling primitive datetime,
serde-json and unique debug implementation.

## Tooling

For added support of wasm, use `wasm-pack` and the commands mentioned below in
the builds section.

- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)

## Builds

- Building wasm

  ```bash
  wasm-pack build -t web --no-typescript --no-pack
  ```

  This command will build the necessary .js and .wasm file from the project
  containing a single `parse` function which performs the
  translation from rust debug logs to json
