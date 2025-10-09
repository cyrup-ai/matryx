# OpenID Connect Standard Scopes

**Source:** https://openid.net/specs/openid-connect-core-1_0.html

## Standard OIDC Scopes

### 1. `openid` (REQUIRED)
- **Required** for all OpenID Connect requests
- Indicates that the application intends to use OIDC protocol to verify user identity
- Without this scope, the behavior is entirely unspecified

### 2. `profile`
- Requests access to default profile claims:
  - `name`
  - `family_name`
  - `given_name`
  - `middle_name`
  - `nickname`
  - `preferred_username`
  - `profile` (URL)
  - `picture` (URL)
  - `website`
  - `gender`
  - `birthdate`
  - `zoneinfo`
  - `locale`
  - `updated_at`

### 3. `email`
- Provides access to user's primary email address
- Claims: `email`, `email_verified`

### 4. `address`
- Provides access to user's physical address
- Claims returned from UserInfo endpoint
- Claim: `address` (JSON object with street_address, locality, region, postal_code, country)

### 5. `phone`
- Provides access to user's phone number
- Claims: `phone_number`, `phone_number_verified`

### 6. `offline_access`
- Gives app access to resources on behalf of user for extended time
- Results in a refresh token being issued
- Allows access when user is not present

## Scope Validation Rules

- **Scope values that are not understood SHOULD be ignored**
- Scopes are space-delimited strings
- Each scope returns a set of user attributes (claims)

## Implementation for MaxTryX

Clients should be allowed to request combinations of:
- `openid` (if using OIDC)
- `profile` (for user profile access)
- `email` (for email access)
- Matrix-specific scopes (see matrix_oauth2_scopes.md)
