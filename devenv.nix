{
  pkgs,
  lib,
  config,
  inputs,
  ...
}: {
  packages = with pkgs; [
    git
    perf
    heaptrack
    (pkgs.python3.withPackages (ps: [
      ps.networkx
      ps.pyparsing
    ]))
  ];
  languages = {
    rust = {
      enable = true;
      channel = "nightly";
      mold.enable = true;
    };
    python = {
      enable = true;
      venv = {
        enable = true;
        requirements = "cnfgen";
      };
    };
  };
}
