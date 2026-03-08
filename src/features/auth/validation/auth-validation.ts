import type { LoginPayload, RegisterPayload } from '../models/auth'

const nicknamePattern = /^[A-Za-z0-9_]+$/

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

  if (!nicknamePattern.test(payload.nickname)) {
    return 'Ник может содержать только английские буквы, цифры и нижнее подчеркивание (_).'
  }

  return null
}

export function validateLoginForm(payload: LoginPayload): string | null {
  if (!payload.identity || !payload.password) {
    return 'Заполните логин и пароль.'
  }

  return null
}