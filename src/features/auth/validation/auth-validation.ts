import type { LoginPayload, RegisterPayload } from '../models/auth'

export function validateRegisterForm(payload: RegisterPayload): string | null {
  if (!payload.email || !payload.nickname || !payload.password || !payload.repeatPassword) {
    return 'Заполните все поля регистрации.'
  }

  if (payload.password !== payload.repeatPassword) {
    return 'Пароли не совпадают.'
  }

  if (payload.nickname.length < 3) {
    return 'Ник должен содержать минимум 3 символа.'
  }

  return null
}

export function validateLoginForm(payload: LoginPayload): string | null {
  if (!payload.identity || !payload.password) {
    return 'Заполните логин и пароль.'
  }

  return null
}
