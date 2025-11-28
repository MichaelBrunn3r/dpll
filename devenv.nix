{
  pkgs,
  lib,
  config,
  inputs,
  ...
}: {
  packages = [pkgs.git];
  languages.rust = {
    enable = true;
    channel = "nightly";
    mold.enable = true;
  };
}
