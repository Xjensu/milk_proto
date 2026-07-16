import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { PasswordChange } from "./PasswordChange";

describe("PasswordChange", () => {
  it("explains the required action and exposes both password fields", () => {
    const markup = renderToStaticMarkup(<PasswordChange email="user@example.test" onChanged={() => undefined} onLogout={() => undefined} />);
    expect(markup).toContain("Обновите пароль");
    expect(markup).toContain("Текущий пароль");
    expect(markup).toContain("Новый пароль");
    expect(markup).toContain("user@example.test");
  });
});
