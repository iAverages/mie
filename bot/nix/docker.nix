{
  pkgs,
  version,
  app,
  runtimeDeps,
  ...
}:
pkgs.dockerTools.buildLayeredImage {
  name = "mie";
  tag = version;
  config.Cmd = ["${app}/bin/mie"];
  contents =
    [
      app
    ]
    ++ runtimeDeps;
}
