export function devAdminPreview(isDevelopment: boolean, search: string) {
  return isDevelopment && new URLSearchParams(search).get("admin-preview") === "1";
}
