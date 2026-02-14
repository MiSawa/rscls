[![crates.io](https://img.shields.io/crates/v/rscls.svg)](https://crates.io/crates/rscls)

# RSCLS

A [language server](https://microsoft.github.io/language-server-protocol/) for [rust-script](https://rust-script.org/).

## How it works

Internally, RSCLS spawns an instance of _rust-analyzer_ with no package configuration. Every time RSCLS receives `textDocument/didOpen` request from the client with `rust-script`, `rust_script` or `rustscript` language id, it changes the language id to `rust`, run _rust-script_ to obtain the project directory and setup `linkedProject` for the project.

## What doesn't work

- Does NOT work on templated rust-scripts, including those need `main` function added.
  - Current implementation doesn't translate file paths nor positions in a file. Since templated rust-scripts are not valid as rust program, we can't handle them directly.
- Commands may not work properly.
- Currently, minimum supported _rust-script_ version is `0.28.0`.

## Install

```shell
cargo install rscls
```

You can alternatively clone this repository to your local, maybe modify some code and run

```shell
cargo install --path path-to-cloned-dir
```

to install locally modified version of the code.

## Uninstall

```shell
cargo uninstall rscls
```

## Example configuration

Here's an example configuration for [neovim](https://github.com/neovim/neovim). I don't use other editor/IDEs, so please figure them out on your own. Pull requests are welcomed!

```lua
-- Assumes `autocmd BufEnter *.ers  setlocal filetype=rustscript` or similar
-- To support files without extension that has shebang, refer to `:help new-filetype-scripts`

-- in wherever you configure lsp, add 'rscls' to the list of lsps to enable.
vim.lsp.enable({ 'rscls' })

-- in after/lsp/rscls.lua

---@type vim.lsp.Config
return {
  -- you may specify command line arguments if needed,
  -- e.g. to specify particular version of rust-analyzer.
  -- See: `rscls --help`
  cmd = { 'rscls' },
  filetypes = { 'rustscript' },
  root_dir = function(bufnr, on_dir)
    local fname = vim.api.nvim_buf_get_name(bufnr)
    on_dir(vim.fs.dirname(fname))
  end,
  settings = {
    -- configurations for the backing rust-analyzer
    ['rust-analyzer'] = {
      imports = {
        group = {
          enable = true,
        },
        granularity = {
          enforce = true,
          group = 'crate',
        },
      },
      cargo = {
        buildScripts = {
          enable = true,
        },
      },
      procMacro = {
        enable = true,
      },
    },
  },
}
```

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed under the terms of both the Apache License, Version 2.0 and the MIT license without any additional terms or conditions.

