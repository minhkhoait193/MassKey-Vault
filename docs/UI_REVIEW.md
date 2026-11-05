# MassKey Vault UI direction

## Screen and flow map

| Flow | Surface | Primary task |
| --- | --- | --- |
| Access | Create, unlock, recover, Recovery Key setup | Enter the vault safely |
| Browse | All accounts, category, search, empty results | Find an account quickly |
| Manage | Account detail, account editor, category editor | View or change one record |
| Protect | Security settings, Vault Health, password and Recovery Key changes | Control access and review risk |
| Transfer | Encrypted ZIP export, import and preview | Move or restore data deliberately |
| Restore | Trash | Recover a deleted account |

## Current assessment

- The established purple and dark palette identifies the product, but the large collection banner and generous gaps push account results below the first viewport.
- Categories, utility navigation and lock controls compete for a fixed sidebar height. Category scrolling must stay independent so the create and lock controls remain reachable.
- The account, category, backup and recovery dialogs share a visual container but need consistent keyboard focus and visible action hierarchy.
- Settings, backup and trash are related tasks. Their shared navigation should be obvious inside Settings while keeping each operation separate.
- Each account card had its own delayed entrance animation, which made a populated list feel slower to scan.

## Direction

Use a compact private workspace: keep the existing brand, palette and terminology; let records and actions occupy the first viewport. Use the same spacing and surface rules across lists, settings and dialogs. Reserve purple for selected navigation, focus and primary actions. Keep destructive and recovery actions explicit.

## Verification boundary

Frontend build and Rust unit tests can verify code and data boundaries. A live Windows review needs a disposable test vault and must cover normal and narrow windows, light and dark themes, keyboard focus, image icon selection, failed import, and recovery. Do not use production vault data for this review.
