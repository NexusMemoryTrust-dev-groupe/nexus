-- V32: Audit append-only enforcement (plan 4.5)
-- The decision journal must be immutable: once an event is appended it can
-- never be edited or deleted — otherwise the audit trail loses its
-- compliance value ("prove the team knew and why it decided so"). Two
-- triggers make UPDATE/DELETE fail hard instead of silently corrupting the
-- trail.
CREATE TRIGGER IF NOT EXISTS trg_audit_no_update
BEFORE UPDATE ON audit_events
BEGIN
    SELECT RAISE(ABORT, 'audit_events is append-only: UPDATE forbidden');
END;

CREATE TRIGGER IF NOT EXISTS trg_audit_no_delete
BEFORE DELETE ON audit_events
BEGIN
    SELECT RAISE(ABORT, 'audit_events is append-only: DELETE forbidden');
END;
