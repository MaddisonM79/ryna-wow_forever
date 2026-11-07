import { describe, expect, it } from "vitest";
import { releaseConfig } from "./release-config.mjs";

const azure = {
  AZURE_TRUSTED_SIGNING_ENDPOINT: "https://eus.codesigning.azure.net",
  AZURE_TRUSTED_SIGNING_ACCOUNT: "acct",
  AZURE_TRUSTED_SIGNING_PROFILE: "prof",
  AZURE_CLIENT_ID: "c",
  AZURE_TENANT_ID: "t",
  AZURE_CLIENT_SECRET: "s",
};

describe("the build's additions", () => {
  it("adds nothing without a key or secrets: the plain unsigned build", () => {
    const r = releaseConfig({ command: "build", pubkey: null, env: {} });
    expect(r.config).toBeNull();
    expect(r.notes.join("\n")).toContain("updates aren't set up");
  });

  it("bakes in the key, and makes artifacts only with the private key too", () => {
    expect(releaseConfig({ command: "build", pubkey: "PUB\n", env: {} }).config).toEqual({
      plugins: { updater: { pubkey: "PUB" } },
    });
    expect(
      releaseConfig({ command: "build", pubkey: "PUB", env: { TAURI_SIGNING_PRIVATE_KEY: "k" }, platform: "win32" }).config,
    ).toEqual({ plugins: { updater: { pubkey: "PUB" } }, bundle: { createUpdaterArtifacts: true } });
    // macOS: the .app.tar.gz needs the "app" target beside the .dmg.
    expect(
      releaseConfig({ command: "build", pubkey: "PUB", env: { TAURI_SIGNING_PRIVATE_KEY: "k" }, platform: "darwin" }).config,
    ).toEqual({
      plugins: { updater: { pubkey: "PUB" } },
      bundle: { createUpdaterArtifacts: true, targets: ["app", "dmg"] },
    });
  });

  it("never makes artifacts from a private key without the public half", () => {
    const r = releaseConfig({ command: "build", pubkey: "  ", env: { TAURI_SIGNING_PRIVATE_KEY: "k" } });
    expect(r.config).toBeNull();
  });

  it("treats an empty secret as absent (an unset GitHub secret is an empty string)", () => {
    const r = releaseConfig({ command: "build", pubkey: "PUB", env: { TAURI_SIGNING_PRIVATE_KEY: "" } });
    expect(r.config).toEqual({ plugins: { updater: { pubkey: "PUB" } } });
  });

  it("signs on Windows only with every Artifact Signing name and credential", () => {
    const full = releaseConfig({ command: "build", pubkey: null, env: azure }).config;
    expect(full).toEqual({
      bundle: {
        windows: {
          signCommand: {
            cmd: "artifact-signing-cli",
            args: ["-e", "https://eus.codesigning.azure.net", "-a", "acct", "-c", "prof", "-d", "Ryna", "%1"],
          },
        },
      },
    });
    const { AZURE_CLIENT_SECRET: _, ...noSecret } = azure;
    expect(releaseConfig({ command: "build", pubkey: null, env: noSecret }).config).toBeNull();
    const { AZURE_TRUSTED_SIGNING_PROFILE: __, ...noProfile } = azure;
    const partial = releaseConfig({ command: "build", pubkey: null, env: noProfile });
    expect(partial.config).toBeNull();
    expect(partial.notes.join("\n")).toContain("missing AZURE_TRUSTED_SIGNING_PROFILE");
  });

  it("gives dev the key only, and leaves other commands alone", () => {
    expect(
      releaseConfig({ command: "dev", pubkey: "PUB", env: { TAURI_SIGNING_PRIVATE_KEY: "k", ...azure } }).config,
    ).toEqual({ plugins: { updater: { pubkey: "PUB" } } });
    expect(releaseConfig({ command: "icon", pubkey: "PUB", env: {} }).config).toBeNull();
    expect(releaseConfig({ command: "signer", pubkey: "PUB", env: {} }).config).toBeNull();
  });
});
