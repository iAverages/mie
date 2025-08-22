{
  pkgs,
  version,
  app,
  runtimeDeps,
  ...
}:
pkgs.dockerTools.buildLayeredImage {
  name = "mie-api";
  tag = version;
  config.Cmd = ["${app}/bin/api"];
  contents =
    [
      app
    ]
    ++ runtimeDeps;
}
