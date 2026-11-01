import {describe,expect,it} from "vitest";
import {getUpdateUrl,UPDATE_RELEASES_URL} from "./updates";

describe("application update links",()=>{
 it("prefers the platform installer selected by the backend",()=>{
  expect(getUpdateUrl({
   update_download_url:"https://github.com/valryte-dev/everquest-loot-tracker-v3/releases/download/v3.21.0/EverQuest.Loot.Tracker_3.21.0_x64-setup.exe",
   update_release_url:"https://github.com/valryte-dev/everquest-loot-tracker-v3/releases/tag/v3.21.0",
  })).toContain("x64-setup.exe");
 });
 it("falls back to the trusted release page and rejects unrelated URLs",()=>{
  const release="https://github.com/valryte-dev/everquest-loot-tracker-v3/releases/tag/v3.21.0";
  expect(getUpdateUrl({update_release_url:release})).toBe(release);
  expect(getUpdateUrl({update_download_url:"https://example.com/setup.exe",update_release_url:"https://example.com/release"})).toBe(UPDATE_RELEASES_URL);
 });
});