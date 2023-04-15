[![crates.io](https://img.shields.io/crates/v/rscls.svg)](https://crates.io/crates/rscls)

# RSCLS
A **proof-of-concept** [language server](https://microsoft.github.io/language-server-protocol/) for [rust-script](https://rust-script.org/).

## How it works
Internally, RSCLS spawns an instance of _rust-analyzer_ on the package directory generated by _rust-script_, and proxy communications between the client (your editor) and _rust-analyzer_.
It then inject [`"rust-analyzer.linkedProjects"` configuration](https://github.com/rust-lang/rust-analyzer/blob/master/docs/user/manual.adoc#non-cargo-based-projects) to the `initialize` request and `workspace/configuration` responses to let _rust-analyzer_ know the dependencies of the script.

## What doesn't work
- Does NOT work on templated rust-scripts
  - Since templated rust-scripts are not valid as rust program, this can't be handled with `rust-project.json`-based approach.
- Commands are not translated. I actually don't know how it is used yet.
- ... maybe plenty of other things doesn't work.
- ... and many TODOs.

## Example configuration
Here's an example configuration for [nvim-lspconfig](https://github.com/neovim/nvim-lspconfig). I don't use other editor/IDEs, so please figure them out on your own.
```lua
-- Assumes `autocmd BufEnter *.ers  setlocal filetype=rust-script`
local lsp_configs = require 'lspconfig.configs'
if not lsp_configs.rlscls then
    lsp_configs.rlscls = {
        default_config = {
            filetypes = { 'rust-script' },
            root_dir = function(fname)
                return lspconfig.util.path.dirname(fname)
            end,
            detached = false,
            get_language_id = function(bufnr, filetype)
                if filetype == 'rust-script' then
                    return 'rust'
                end
            end,
        },
        on_new_config = function(new_config, root_dir)
            if not new_config.cmd then
                local bufname = vim.api.nvim_buf_get_name(0)
                new_config.cmd = {
                    'rscls',
                    bufname,
                }
            end
        end,
        docs = {
            description = [[
An awesome documentation here.
]],
        }
    }
end
lspconfig.rlscls.setup { }
```

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed under the terms of both the Apache License, Version 2.0 and the MIT license without any additional terms or conditions.

