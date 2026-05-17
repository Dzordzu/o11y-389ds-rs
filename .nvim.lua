-- Note: To make it work use `set ex` in the nvim config
-- Note: add this function to ~/.config/nvim/config.lua:
--
-- ```lua
-- vim.g.load_rust_lsp = function(settings)
--    require 'lspconfig'.rust_analyzer.setup {
--       on_attach = CUSTOM_ATTACH,
--       settings = {
--          ["rust-analyzer"] = settings,
--       },
--       tools = {
--          inlay_hints = {
--             auto = true
--          }
--       },
--    }
-- end
-- ```

if vim.g.load_rust_lsp ~= nil then
   vim.g.load_rust_lsp({
      cargo = {
         features = "all",
         buildScripts = {
            enable = true,
         },
      },
      trace = {
         server = "verbose",
      },
   })
end

