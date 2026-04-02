# Business Logic Questions Log

### Inventory value threshold for manager approval
**Question**: The prompt states managers must approve any document that changes inventory value by more than $2,500.00. Should this apply to the *total* document value or individual line item value?  
**My Understanding**: Given the risk focus, the threshold should apply to the aggregate value of all items within a single inventory document.  
**Solution**: Implemented a check during document creation that flags `requires_manager_approval = true` if the sum of `quantity * unit_value_cents` across all lines exceeds 250,000 cents ($2,500.00).

### Recommendation "Why" Explanations
**Question**: The prompt requires a short "why you're seeing this" explanation for each recommendation. How should these be generated if the system is offline?  
**My Understanding**: The explanation should reflect the source data (e.g., "Based on your interest in [Category]" or "Similar to your favorite [Device]").  
**Solution**: Added a `reason` field to the recommendation model that is populated at runtime based on the user's recent activity history (views or favorites).

### After-Sales Evidence Persistence
**Question**: When a user attaches evidence to an after-sales case, should the media be uniquely linked to that case or can it be reused across the marketplace?  
**My Understanding**: Evidence should be immutable and specifically tied to the support case for audit integrity.  
**Solution**: Created an `after_sales_evidence` join table that links a support case to a generic `listing_media` record, ensuring the media persists even if a related listing is deleted.

### Shipping Integration "Points"
**Question**: The prompt mentions tracking API "integration points" that remain disabled by default. Does this mean the code should include placeholders for external API calls?  
**My Understanding**: The system must remain strictly offline. The "integration points" should be logical toggles in the database that, if enabled, would trigger code blocks for external requests (which currently remain un-implemented or mocked to maintain offline compliance).  
**Solution**: Added an `integration_enabled` flag to the `shipment_orders` table. The backend logic checks this flag but currently only performs local state transitions to ensure zero external network calls.

### Taxonomy Level Depth
**Question**: How many levels should the multi-level taxonomy support?  
**My Understanding**: A standard 3-level depth (Category > Subcategory > Item Type) is typically sufficient for organizational device management.  
**Solution**: The `taxonomy_nodes` table includes a `level` field and `parent_id` to support recursive hierarchies of arbitrary depth, though the UI is currently optimized for a 3-level display.
