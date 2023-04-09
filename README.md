# RSCLS
A **proof-of-concept** [language server](https://microsoft.github.io/language-server-protocol/) for [rust-script](https://rust-script.org/).

## How it works
Internally, RSCLS spawns an instance of _rust-analyzer_ on the package directory generated by _rust-script_, and proxy communications between the client (your editor) and _rust-analyzer_.
While proxying request/response/notifications, when it saw an URI that represents the rust-script sent from the client, it translates the URI to the source file in the generated package directory. And similary, when it saw an URI that represents the source file in the generated package directory sent from the server, it translates the URI to the original rust-script file.

## What doesn't work
- Shebang (`#!/usr/bin/env rust-script` thingy) is unfortunately not supported (yet). This is because _rust-script_ removes this shebang line when it creates the package, which results in disagreement of the line number that the server and the client sees.
- Does NOT work on templated rust-scripts
  - Since templated rust-scripts are not valid as rust program, it's really hard to implement conversion. It's not only just add/subtract an offset to each position-typed request/response. For example, think of a code action adding `use std::collections::HashMap`. _rust-analyzer_ would think it should be added to the top part of the rust file which came from the template, but we'd have to put this to somewhere else.
- Notebook features are unhandled. Does _rust-analyzer_ even support it?
- Commands are not translated. I actually don't know how it is used yet.
- `textDocument/diagnostic`, `workspace/diagnostic`, `workspace/diagnostic/refresh` are not supported yet, as `lsp-types` doesn't have it yet.
- Translation of _Registration Options_ are unsupported yet. Esp. it's hard to translate `DocumentFilter`.
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

