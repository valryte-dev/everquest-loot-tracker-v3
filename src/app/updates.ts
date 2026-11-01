export const UPDATE_RELEASES_URL="https://github.com/valryte-dev/everquest-loot-tracker-v3/releases/latest";
const UPDATE_DOWNLOAD_PREFIX="https://github.com/valryte-dev/everquest-loot-tracker-v3/releases/download/";
const UPDATE_RELEASE_PREFIX="https://github.com/valryte-dev/everquest-loot-tracker-v3/releases/";

export function getUpdateUrl(settings:Record<string,string|undefined>):string{
 const download=settings.update_download_url?.trim();
 if(download?.startsWith(UPDATE_DOWNLOAD_PREFIX))return download;
 const release=settings.update_release_url?.trim();
 return release?.startsWith(UPDATE_RELEASE_PREFIX)?release:UPDATE_RELEASES_URL;
}