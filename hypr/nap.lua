-- Optional Hyprland 0.55+ rule. Load from your existing Lua configuration.
o.window({ class = "^nap$" }, {
  tag = "-default-opacity",
  float = true,
  center = true,
  keep_aspect_ratio = true,
  no_dim = true,
  opacity = "1 1",
})
