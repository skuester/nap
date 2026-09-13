-- Nice Audio Player — installed by nap --install-hyprland or make install.
-- Keep the 720:504 deck ratio when resizing the floating window.
o.window({ class = "^nap$" }, {
  tag = "-default-opacity",
  float = true,
  center = true,
  keep_aspect_ratio = true,
  no_dim = true,
  opacity = "1 1",
})
