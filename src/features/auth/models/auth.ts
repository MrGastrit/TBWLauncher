export type UserRole = 'user' | 'admin'

export type AuthUser = {
  id: string
  nickname: string
  email: string
  skinUrl?: string
  role: UserRole
}

export type AuthTokens = {
  accessToken: string
  refreshToken?: string
}

export type LoginPayload = {
  identity: string
  password: string
}

export type RegisterPayload = {
  email: string
  nickname: string
  password: string
  repeatPassword: string
}

export type UpdateAccountPayload = {
  nickname?: string
  skinPath?: string
}

export type ChangePasswordPayload = {
  currentPassword: string
  nextPassword: string
}

export type AuthResult = {
  user: AuthUser
  tokens: AuthTokens
}
