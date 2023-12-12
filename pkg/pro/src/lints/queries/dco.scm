; © Steven Kalt
; SPDX-License-Identifier: Polyform-Noncommercial-1.0.0 OR LicenseRef-PolyForm-Free-Trial-1.0.0
(
  (trailer (token) @token) @required
  (#match? @token "^Signed-off-by$")
)
