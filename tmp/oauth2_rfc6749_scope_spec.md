# OAuth 2.0 Scope Specification (RFC 6749 Section 3.3)

**Source:** https://datatracker.ietf.org/doc/html/rfc6749#section-3.3

## Scope Format

- Scopes are **space-delimited, case-sensitive strings**
- Order of scopes does not matter
- Each scope string adds an additional access range to the requested scope

**ABNF Syntax:**
```
scope = scope-token *( SP scope-token )
scope-token = 1*( %x21 / %x23-5B / %x5D-7E )
```

## Scope Validation Rules

1. **Authorization Server MAY:**
   - Fully or partially ignore the scope requested by the client
   - Based on authorization server policy or resource owner's instructions

2. **If issued scope differs from requested:**
   - MUST include the 'scope' response parameter
   - To inform the client of the actual scope granted

3. **If client omits scope parameter:**
   - Server MUST either:
     a. Process the request using a pre-defined default value, OR
     b. Fail the request indicating an invalid scope

4. **Documentation:**
   - Server SHOULD document its scope requirements
   - Server SHOULD document default value (if defined)

## Implementation Notes for MaxTryX

- Parse scope strings by splitting on space characters
- Validate each requested scope against client's allowed scopes
- Reject requests with invalid scopes OR grant subset of valid scopes
- Document supported scopes in API documentation
