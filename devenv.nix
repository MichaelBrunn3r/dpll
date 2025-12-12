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
    (pkgs.python3.withPackages (ps:
      with ps; [
        networkx
        pyparsing
        numpy
        matplotlib
        plotly
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

  # Optional: Create a handy alias to run the plotter
  scripts.plot-stats.exec = "python plot_metrics.py";
}
