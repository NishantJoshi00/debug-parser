## Tooling

- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)

## Builds

- Building wasm

  ```bash
  wasm-pack build -t web --no-typescript --no-pack
  ```

  This command will build the necessary .js and .wasm file from the project
  containing a single `parse` function which performs the
  translation from rust debug logs to json
