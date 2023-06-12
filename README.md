[![crates.io](https://img.shields.io/crates/v/rscls.svg)](https://crates.io/crates/rscls)

# RSCLS
A **proof-of-concept** [language server](https://microsoft.github.io/language-server-protocol/) for [rust-script](https://rust-script.org/).

## How it works
Internally, RSCLS spawns an instance of _rust-analyzer_ with no package configuration. Every time RSCLS receives `textDocument/didOpen` request from the client with `rust-script` language id, it changes the language id to `rust`, run _rust-script_ to obtain the project directory and setup `linkedProject` for the project.

## What doesn't work
- Does NOT work on templated rust-scripts, including those need `main` function added.
  - Current implementation doesn't translate file paths nor positions in a file. Since templated rust-scripts are not valid as rust program, we can't handle them directly.
- Commands may not work properly.

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
Here's an example configuration for [nvim-lspconfig](https://github.com/neovim/nvim-lspconfig). I don't use other editor/IDEs, so please figure them out on your own. Pull requests are welcomed!
```lua
-- Assumes `autocmd BufEnter *.ers  setlocal filetype=rust-script` or similar
local lsp_configs = require 'lspconfig.configs'
if not lsp_configs.rlscls then
    lsp_configs.rlscls = {
        default_config = {
            cmd = { 'rscls' },
            filetypes = { 'rust-script' },
            root_dir = function(fname)
                return lspconfig.util.path.dirname(fname)
            end,
        },
        docs = {
            description = [[
https://github.com/MiSawa/rscls

rscls, a language server for rust-script
]],
        }
    }
end
lspconfig.rlscls.setup {
    settings = {
        ['rust-analyzer'] = {
            imports = {
                group = {
                    enable = true,
                },
                granularity = {
                    enforce = true,
                    group = "crate",
                },
            },
            cargo = {
                buildScripts = {
                    enable = true,
                },
            },
            procMacro = {
                enable = true,
            }
        },
    }
}
```

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed under the terms of both the Apache License, Version 2.0 and the MIT license without any additional terms or conditions.

