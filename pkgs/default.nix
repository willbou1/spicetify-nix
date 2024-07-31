{ pkgs, self }:
let
  inherit (pkgs) lib;
  spicePkgs = self.legacyPackages.${pkgs.stdenv.system};
  json = lib.importJSON "${self}/generated.json";

  makeExtension = v: {

    inherit (v) name main;

    outPath =
      if v.main == "__INCLUDE__" then
        pkgs.fetchurl v.source
      else
        pkgs.fetchzip (v.source // { extension = "tar"; });
  };
in
{
  inherit (json) snippets;

  fetcher = pkgs.callPackage ./fetcher { inherit self; };
  spicetify = pkgs.callPackage "${self}/pkgs/spicetify.nix" { };
  spicetifyBuilder = pkgs.callPackage "${self}/pkgs/spicetifyBuilder.nix" {
    inherit (spicePkgs) spicetify;
  };

  extensions =
    let
      prev = lib.mapAttrs (n: v: makeExtension v) json.extensions;
    in
    prev
    // {
      # Overrides go here
    };

  themes =
    let
      prev = lib.mapAttrs (n: v: {
        inherit (v) name usercss schemes;
        include = map makeExtension v.include;
        outPath = (pkgs.fetchzip v.source);
      }) json.themes;
    in
    prev
    // {
      # Overrides go here
    };

  # not possible to auto generate all the apps right now

  #  apps = lib.mapAttrs (n: v: {
  #    inherit (v) name;
  #    outPath = (pkgs.fetchurl v.source);
  #  }) json.apps;

}
