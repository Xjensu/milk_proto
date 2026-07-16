import { useState, type FormEvent } from "react";
import { ShieldCheck } from "lucide-react";

import { Badge, Button, Panel } from "../../components/ui";
import { accountFromAccessToken, api, type Account } from "../../lib/api";

export function PasswordChange({ email, onChanged, onLogout }: { email: string; onChanged: (account: Account) => void; onLogout: () => void }) {
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (newPassword !== confirmation) { setError("Новые пароли не совпадают."); return; }
    setLoading(true);
    setError("");
    try {
      const response = await api.changePassword(currentPassword, newPassword);
      sessionStorage.setItem("milk_schedule_access_token", response.access_token);
      sessionStorage.setItem("milk_schedule_refresh_token", response.refresh_token);
      onChanged(accountFromAccessToken(response.access_token, email));
    } catch {
      setError("Не удалось обновить пароль. Проверьте текущий пароль и требования безопасности.");
    } finally { setLoading(false); }
  };

  return <section className="auth-page section-wrap"><div className="auth-intro" data-reveal><Badge tone="accent"><ShieldCheck size={14} /> Обязательное действие</Badge><h1>Обновите пароль<span>.</span></h1><p>Перед продолжением установите личный пароль. Это защищает временно созданную учётную запись.</p><button className="text-link" onClick={onLogout}>Выйти из аккаунта</button></div><Panel className="auth-card" data-reveal><div className="auth-card-heading"><span className="eyebrow">Milk Schedule / Security</span><h2>Защита аккаунта</h2><p>{email}</p></div><form onSubmit={submit}><label>Текущий пароль<input required type="password" autoComplete="current-password" value={currentPassword} onChange={(event) => setCurrentPassword(event.target.value)} /></label><label>Новый пароль<input required minLength={8} type="password" autoComplete="new-password" value={newPassword} onChange={(event) => setNewPassword(event.target.value)} /></label><label>Повторите новый пароль<input required minLength={8} type="password" autoComplete="new-password" value={confirmation} onChange={(event) => setConfirmation(event.target.value)} /></label>{error ? <div className="form-error" role="alert">{error}</div> : null}<Button className="button-primary button-full" type="submit" disabled={loading}>{loading ? "Обновляем…" : "Сохранить и продолжить"}</Button></form></Panel></section>;
}
