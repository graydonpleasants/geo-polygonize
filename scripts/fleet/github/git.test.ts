import { expect, test, describe } from "bun:test";
import { parseGitRemoteUrl } from "./git";

describe("parseGitRemoteUrl", () => {
  test("parses standard HTTPS URLs with .git", () => {
    expect(parseGitRemoteUrl("https://github.com/owner/repo.git")).toEqual({
      owner: "owner",
      repo: "repo",
      fullName: "owner/repo",
    });
  });

  test("parses standard HTTPS URLs without .git", () => {
    expect(parseGitRemoteUrl("https://github.com/owner/repo")).toEqual({
      owner: "owner",
      repo: "repo",
      fullName: "owner/repo",
    });
  });

  test("parses HTTP URLs", () => {
    expect(parseGitRemoteUrl("http://github.com/owner/repo.git")).toEqual({
      owner: "owner",
      repo: "repo",
      fullName: "owner/repo",
    });
  });

  test("parses standard SSH URLs with .git", () => {
    expect(parseGitRemoteUrl("git@github.com:owner/repo.git")).toEqual({
      owner: "owner",
      repo: "repo",
      fullName: "owner/repo",
    });
  });

  test("parses standard SSH URLs without .git", () => {
    expect(parseGitRemoteUrl("git@github.com:owner/repo")).toEqual({
      owner: "owner",
      repo: "repo",
      fullName: "owner/repo",
    });
  });

  test("throws on unsupported domains", () => {
    expect(() => parseGitRemoteUrl("https://gitlab.com/owner/repo.git")).toThrow(
      "Unable to parse git remote URL: https://gitlab.com/owner/repo.git"
    );
  });

  test("throws on malformed URLs", () => {
    expect(() => parseGitRemoteUrl("not-a-url")).toThrow(
      "Unable to parse git remote URL: not-a-url"
    );
  });
});
