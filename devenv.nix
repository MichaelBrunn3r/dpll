{
  pkgs,
  lib,
  config,
  inputs,
  ...
}: {
  packages = with pkgs; [git perf];
  languages.rust = {
    enable = true;
    channel = "nightly";
    mold.enable = true;
  };
}
