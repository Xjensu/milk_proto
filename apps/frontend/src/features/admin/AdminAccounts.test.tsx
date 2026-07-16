import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { AdminAccounts } from "./AdminAccounts";

describe("AdminAccounts", () => {
  it("renders account management backed by the live account collection", () => {
    const markup = renderToStaticMarkup(<AdminAccounts />);
    expect(markup).toContain("Управление аккаунтами");
    expect(markup).toContain("Создать аккаунт");
    expect(markup).toContain("Загрузка аккаунтов");
    expect(markup).not.toContain("будет загружен");
  });
});
