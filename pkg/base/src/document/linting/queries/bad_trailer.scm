; © Steven Kalt
; SPDX-License-Identifier: APACHE-2.0
(
  (trailer (token) @token (value)? @forbidden)
  (#match? @forbidden "^\\s*$")
)
