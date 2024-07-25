{
  lib,
  buildGoModule,
  fetchFromGitHub,
}:
let
  version = "2.36.15";
in
buildGoModule {
  pname = "spicetify";
  inherit version;

  src = fetchFromGitHub {
    owner = "spicetify";
    repo = "cli";
    rev = "v${version}";
    hash = "sha256-K3lhPeW6L9N9OljNn8e6iQqx4k4HX5A9mX7SUlv2IRw=";
  };

  vendorHash = "sha256-i3xnf440lslzeDJ4pLLONqw8ACbdkKgPIhlPSuC1Vng=";

  ldflags = [
    "-s -w"
    "-X 'main.version=Dev'"
  ];

  CGO_ENABLED = "0";

  postInstall = ''
    mv $out/bin/cli $out/bin/spicetify
    ln -s $src/jsHelper $out/bin/jsHelper
    ln -s $src/css-map.json $out/bin/css-map.json
  '';

  meta = {
    description = "Command-line tool to customize Spotify client";
    homepage = "https://github.com/spicetify/cli";
    license = lib.licenses.gpl3Plus;
    mainProgram = "spicetify";
  };
}
