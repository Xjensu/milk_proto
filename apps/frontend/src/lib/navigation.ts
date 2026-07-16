import type { Account } from "./api";

export type StaffView = "login" | "password" | "admin" | "self";

export function staffView(account: Account | null): StaffView {
  if (!account) return "login";
  if (account.must_change_password) return "password";
  return account.role === "teacher" ? "self" : "admin";
}
