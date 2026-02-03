# Business Operations Use Cases

AI agents for automating business operations, reducing manual work, and improving response times across customer support, sales, finance, legal, HR, and customer success workflows.

## Table of Contents

1. [Overview](#overview)
2. [Architecture Summary](#architecture-summary)
3. [Use Cases](#use-cases)
   - [Customer Support Ticket Routing](#1-customer-support-ticket-routing-and-response)
   - [Sales Lead Qualification](#2-sales-lead-qualification-and-crm-updates)
   - [Invoice Processing](#3-invoice-processing-and-ap-automation)
   - [Contract Review](#4-contract-review-and-compliance-monitoring)
   - [Employee Onboarding](#5-employee-onboarding-workflow-automation)
   - [Meeting Summarization](#6-meeting-summarization-and-action-item-tracking)
   - [Customer Health Monitoring](#7-customer-health-monitoring-and-churn-prediction)
4. [Integration Patterns](#integration-patterns)
5. [Connector Configurations](#connector-configurations)
6. [ROI and Efficiency Metrics](#roi-and-efficiency-metrics)

---

## Overview

AgentZero enables organizations to deploy AI agents that automate routine business operations while maintaining human oversight for critical decisions. The connector architecture allows seamless integration with existing business tools.

**Key Benefits:**
- **Reduce response times** from hours to seconds for routine inquiries
- **Eliminate manual data entry** across CRM, ERP, and ticketing systems
- **Ensure consistency** in processes that previously varied by employee
- **Scale operations** without proportional headcount increases
- **24/7 availability** for time-sensitive workflows

---

## Architecture Summary

### Trigger Flow

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   External      │────▶│   AgentZero     │────▶│   Business      │
│   System        │     │   Gateway       │     │   Tool          │
│ (Zendesk/Slack) │     │   :18791        │     │ (Salesforce)    │
└─────────────────┘     └─────────────────┘     └─────────────────┘
        │                       │
        │ POST /api/gateway/submit
        ▼                       │ respond_to: ["salesforce-connector"]
   ┌─────────────────┐          ▼
   │ {               │    ┌─────────────────┐
   │   agent_id,     │    │  Webhook POST   │
   │   message,      │    │  to your service│
   │   respond_to[], │    └─────────────────┘
   │   metadata{}    │
   │ }               │
   └─────────────────┘
```

### Submit Endpoint

```bash
POST http://localhost:18791/api/gateway/submit
Content-Type: application/json

{
  "agent_id": "support-router",
  "message": "Customer ticket: Cannot access my account after password reset",
  "respond_to": ["zendesk-connector", "slack-alerts"],
  "metadata": {
    "ticket_id": "TKT-12345",
    "customer_email": "user@example.com",
    "priority": "high",
    "source": "zendesk"
  },
  "thread_id": "thread-abc123",
  "external_ref": "zendesk:TKT-12345"
}
```

### Webhook Response Format

When an agent completes, connectors receive:

```json
{
  "context": {
    "session_id": "sess-abc123",
    "thread_id": "thread-abc123",
    "agent_id": "support-router",
    "timestamp": "2024-01-15T09:00:00Z"
  },
  "capability": "respond",
  "payload": {
    "message": "Ticket routed to Technical Support team. Suggested response: ...",
    "execution_id": "exec-xyz789",
    "conversation_id": "conv-abc123"
  }
}
```

---

## Use Cases

### 1. Customer Support Ticket Routing and Response

Automatically categorize, prioritize, and route incoming support tickets while drafting initial responses.

**Business Value:**
- 70% reduction in first-response time
- 40% decrease in ticket misrouting
- 24/7 ticket triage without staffing costs

#### Trigger: New Ticket from Zendesk

```bash
# Zendesk webhook triggers this when a new ticket is created
curl -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "support-router",
    "message": "New support ticket received:\n\nSubject: Cannot complete checkout - payment failing\nDescription: I have been trying to purchase the Pro plan for 2 hours. My credit card keeps getting declined even though it works everywhere else. Order ID: ORD-98765\n\nCustomer: john.doe@company.com\nAccount Type: Enterprise\nPrevious Tickets: 3 (all resolved)",
    "respond_to": ["zendesk-update", "slack-support-channel"],
    "metadata": {
      "ticket_id": "ZD-45678",
      "requester_email": "john.doe@company.com",
      "account_tier": "enterprise",
      "created_at": "2024-01-15T14:32:00Z"
    },
    "external_ref": "zendesk:ZD-45678"
  }'
```

#### Agent Response via Webhook

```json
{
  "context": {
    "session_id": "sess-support-001",
    "thread_id": null,
    "agent_id": "support-router",
    "timestamp": "2024-01-15T14:32:15Z"
  },
  "capability": "respond",
  "payload": {
    "message": "{\"category\": \"billing\", \"priority\": \"high\", \"assigned_team\": \"billing-support\", \"suggested_response\": \"Hi John, I apologize for the frustration with your checkout. I can see this is affecting your Pro plan upgrade. I have escalated this to our billing team who will investigate the payment processing issue with Order ID ORD-98765. In the meantime, could you try using a different browser or clearing your cache? We will have an update for you within 2 hours.\", \"internal_notes\": \"Enterprise customer with history of resolved tickets. Payment gateway logs should be checked for ORD-98765. Consider offering discount code for inconvenience.\"}",
    "execution_id": "exec-sr-001",
    "conversation_id": "conv-zd45678"
  }
}
```

#### Scheduled Ticket Review (Cron)

```bash
# Create a cron job to review unresolved tickets daily
curl -X POST http://localhost:18791/api/cron \
  -H "Content-Type: application/json" \
  -d '{
    "id": "daily-ticket-review",
    "name": "Daily Unresolved Ticket Review",
    "schedule": "0 0 9 * * 1-5",
    "message": "Review all tickets older than 24 hours that are still in \"pending\" or \"open\" status. Identify patterns, suggest escalations, and flag SLA risks.",
    "respond_to": ["slack-support-managers", "zendesk-bulk-update"],
    "enabled": true
  }'
```

---

### 2. Sales Lead Qualification and CRM Updates

Qualify inbound leads based on company data, engagement signals, and fit scoring, then update CRM records automatically.

**Business Value:**
- 60% reduction in lead qualification time
- 25% improvement in lead-to-opportunity conversion
- Sales reps focus on high-value conversations only

#### Trigger: New Lead from HubSpot

```bash
# HubSpot workflow triggers this when a new lead is created
curl -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "lead-qualifier",
    "message": "Qualify this new lead:\n\nCompany: TechCorp Solutions\nContact: Sarah Chen, VP of Engineering\nEmail: sarah.chen@techcorp.io\nCompany Size: 250 employees\nIndustry: SaaS\nSource: Downloaded whitepaper on \"AI in DevOps\"\nWebsite visits (30d): 12\nPages viewed: Pricing, Enterprise Features, Case Studies\nLinkedIn: Active poster about automation",
    "respond_to": ["hubspot-update", "slack-sales-team"],
    "metadata": {
      "lead_id": "HS-789012",
      "hubspot_company_id": "COMP-456",
      "source_campaign": "devops-whitepaper-q1",
      "captured_at": "2024-01-15T10:15:00Z"
    },
    "external_ref": "hubspot:HS-789012"
  }'
```

#### Agent Response

```json
{
  "context": {
    "session_id": "sess-lead-001",
    "thread_id": null,
    "agent_id": "lead-qualifier",
    "timestamp": "2024-01-15T10:15:30Z"
  },
  "capability": "respond",
  "payload": {
    "message": "{\"qualification_score\": 85, \"qualification_tier\": \"A\", \"recommended_action\": \"immediate_outreach\", \"reasoning\": \"Strong fit indicators: VP-level decision maker, company size in ICP range (100-500), high engagement with pricing and enterprise pages, technical content download suggests active evaluation. LinkedIn activity confirms interest in automation.\", \"suggested_next_steps\": [\"Personal email from AE within 4 hours\", \"Reference case study from similar SaaS company\", \"Offer demo focused on DevOps integration\"], \"crm_updates\": {\"lead_score\": 85, \"lifecycle_stage\": \"MQL\", \"lead_status\": \"qualified\", \"owner\": \"sales-team-enterprise\"}}",
    "execution_id": "exec-lq-001",
    "conversation_id": "conv-hs789012"
  }
}
```

#### Weekly Lead Pipeline Analysis (Cron)

```bash
curl -X POST http://localhost:18791/api/cron \
  -H "Content-Type: application/json" \
  -d '{
    "id": "weekly-pipeline-analysis",
    "name": "Weekly Lead Pipeline Analysis",
    "schedule": "0 0 8 * * 1",
    "message": "Analyze the sales pipeline from the past week. Identify: 1) Leads that have gone cold, 2) Opportunities at risk of stalling, 3) Patterns in lost deals, 4) Top performing lead sources. Provide actionable recommendations for the sales team.",
    "respond_to": ["slack-sales-leadership", "hubspot-report"],
    "enabled": true
  }'
```

---

### 3. Invoice Processing and AP Automation

Extract data from invoices, validate against purchase orders, flag discrepancies, and prepare for approval.

**Business Value:**
- 80% reduction in invoice processing time
- 95% accuracy in data extraction
- 50% fewer payment delays due to errors

#### Trigger: New Invoice Received

```bash
# Email parser or document management system triggers this
curl -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "invoice-processor",
    "message": "Process this invoice:\n\nVendor: Acme Supplies Inc.\nInvoice Number: INV-2024-0892\nInvoice Date: 2024-01-10\nDue Date: 2024-02-10\nAmount: $12,450.00\nPO Reference: PO-2024-0234\n\nLine Items:\n1. Server Hardware (qty: 5) - $10,000.00\n2. Installation Services - $2,000.00\n3. Shipping & Handling - $450.00\n\nPayment Terms: Net 30\nBank Details: [Attached]",
    "respond_to": ["erp-connector", "slack-ap-team", "approval-workflow"],
    "metadata": {
      "document_id": "DOC-INV-0892",
      "vendor_id": "VEND-ACME-001",
      "received_via": "email",
      "attachment_url": "https://storage.internal/invoices/inv-2024-0892.pdf"
    },
    "external_ref": "invoice:INV-2024-0892"
  }'
```

#### Agent Response

```json
{
  "context": {
    "session_id": "sess-inv-001",
    "thread_id": null,
    "agent_id": "invoice-processor",
    "timestamp": "2024-01-15T11:00:15Z"
  },
  "capability": "respond",
  "payload": {
    "message": "{\"validation_status\": \"requires_review\", \"extracted_data\": {\"vendor\": \"Acme Supplies Inc.\", \"invoice_number\": \"INV-2024-0892\", \"total\": 12450.00, \"currency\": \"USD\", \"po_reference\": \"PO-2024-0234\"}, \"po_match\": {\"status\": \"partial_match\", \"po_total\": 11500.00, \"variance\": 950.00, \"variance_reason\": \"Shipping charges not in original PO\"}, \"gl_coding\": {\"6100\": 10000.00, \"6200\": 2000.00, \"6300\": 450.00}, \"flags\": [{\"type\": \"variance\", \"severity\": \"medium\", \"message\": \"Invoice exceeds PO by $950 (8.3%)\"}], \"recommended_action\": \"Route to manager approval due to PO variance\", \"auto_approval_eligible\": false}",
    "execution_id": "exec-ip-001",
    "conversation_id": "conv-inv0892"
  }
}
```

#### Daily Invoice Summary (Cron)

```bash
curl -X POST http://localhost:18791/api/cron \
  -H "Content-Type: application/json" \
  -d '{
    "id": "daily-ap-summary",
    "name": "Daily AP Summary",
    "schedule": "0 0 17 * * 1-5",
    "message": "Generate end-of-day AP summary: 1) Invoices processed today, 2) Pending approvals with aging, 3) Payments due this week, 4) Any invoices with unresolved discrepancies. Highlight urgent items requiring attention.",
    "respond_to": ["slack-finance-team", "email-cfo"],
    "enabled": true
  }'
```

---

### 4. Contract Review and Compliance Monitoring

Analyze contracts for risk clauses, compliance issues, and deviations from standard terms.

**Business Value:**
- 75% faster contract review cycle
- 90% reduction in missed compliance issues
- Standardization of contract language across the organization

#### Trigger: New Contract for Review

```bash
curl -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "contract-reviewer",
    "message": "Review this vendor contract:\n\nContract Type: SaaS Subscription Agreement\nVendor: CloudData Analytics\nContract Value: $150,000/year\nTerm: 3 years with auto-renewal\n\nKey Clauses to Review:\n- Data Processing Agreement (GDPR compliance)\n- Liability cap\n- Termination provisions\n- SLA commitments\n- Indemnification\n- IP ownership\n\nCompare against our standard terms and flag deviations.",
    "respond_to": ["legal-review-queue", "slack-legal-team"],
    "metadata": {
      "contract_id": "CONTRACT-2024-0156",
      "department": "engineering",
      "requester": "mike.johnson@ourcompany.com",
      "document_url": "https://docstore.internal/contracts/clouddata-msa.pdf",
      "urgency": "standard"
    },
    "external_ref": "contract:CONTRACT-2024-0156"
  }'
```

#### Agent Response

```json
{
  "context": {
    "session_id": "sess-contract-001",
    "thread_id": null,
    "agent_id": "contract-reviewer",
    "timestamp": "2024-01-15T14:00:45Z"
  },
  "capability": "respond",
  "payload": {
    "message": "{\"overall_risk_score\": 72, \"risk_level\": \"medium\", \"summary\": \"Contract contains several deviations from standard terms requiring legal review\", \"findings\": [{\"clause\": \"Liability Cap\", \"risk\": \"high\", \"issue\": \"Liability capped at 6 months fees vs our standard 12 months\", \"recommendation\": \"Negotiate to match standard terms\"}, {\"clause\": \"Auto-Renewal\", \"risk\": \"medium\", \"issue\": \"60-day notice required for non-renewal vs our standard 30 days\", \"recommendation\": \"Calendar reminder needed if accepted\"}, {\"clause\": \"Data Processing\", \"risk\": \"low\", \"issue\": \"DPA included and GDPR compliant\", \"recommendation\": \"Acceptable as-is\"}, {\"clause\": \"Termination for Convenience\", \"risk\": \"high\", \"issue\": \"No termination for convenience clause\", \"recommendation\": \"Must negotiate - required by policy\"}], \"compliance_check\": {\"gdpr\": \"pass\", \"sox\": \"pass\", \"hipaa\": \"not_applicable\"}, \"recommended_action\": \"Send to Legal for negotiation on liability cap and termination clause\"}",
    "execution_id": "exec-cr-001",
    "conversation_id": "conv-contract156"
  }
}
```

#### Weekly Compliance Audit (Cron)

```bash
curl -X POST http://localhost:18791/api/cron \
  -H "Content-Type: application/json" \
  -d '{
    "id": "weekly-compliance-audit",
    "name": "Weekly Contract Compliance Audit",
    "schedule": "0 0 6 * * 1",
    "message": "Run weekly compliance audit: 1) Contracts expiring in next 90 days, 2) Contracts with outstanding compliance items, 3) Vendor certifications due for renewal, 4) Any regulatory changes affecting existing contracts. Generate report for legal and compliance teams.",
    "respond_to": ["slack-legal-compliance", "email-compliance-officer"],
    "enabled": true
  }'
```

---

### 5. Employee Onboarding Workflow Automation

Orchestrate the multi-step onboarding process: equipment provisioning, account creation, training assignment, and team introductions.

**Business Value:**
- 50% reduction in time-to-productivity for new hires
- 100% consistency in onboarding experience
- Zero missed onboarding steps

#### Trigger: New Employee Start

```bash
# HRIS system triggers this when employee start date approaches
curl -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "onboarding-coordinator",
    "message": "Initiate onboarding for new employee:\n\nName: Alex Rivera\nStart Date: 2024-01-22\nDepartment: Product Engineering\nRole: Senior Software Engineer\nManager: Jennifer Walsh\nLocation: Remote (US-West)\n\nRequired:\n- MacBook Pro 16\"\n- GitHub access\n- AWS console access\n- Slack workspace\n- Jira/Confluence access\n- Engineering onboarding curriculum\n- 1:1 introductions with team leads",
    "respond_to": ["it-provisioning", "hr-system", "slack-hr-notifications"],
    "metadata": {
      "employee_id": "EMP-2024-0089",
      "hris_record": "WORKDAY-89012",
      "start_date": "2024-01-22",
      "department_code": "ENG-PROD",
      "manager_email": "jennifer.walsh@company.com"
    },
    "external_ref": "onboarding:EMP-2024-0089"
  }'
```

#### Agent Response

```json
{
  "context": {
    "session_id": "sess-onboard-001",
    "thread_id": null,
    "agent_id": "onboarding-coordinator",
    "timestamp": "2024-01-15T09:00:30Z"
  },
  "capability": "respond",
  "payload": {
    "message": "{\"onboarding_plan\": {\"employee\": \"Alex Rivera\", \"start_date\": \"2024-01-22\", \"status\": \"initiated\"}, \"tasks_created\": [{\"task\": \"Equipment Order\", \"assignee\": \"IT-Procurement\", \"due\": \"2024-01-19\", \"status\": \"pending\", \"details\": \"MacBook Pro 16in M3, shipping to home address\"}, {\"task\": \"Account Provisioning\", \"assignee\": \"IT-Security\", \"due\": \"2024-01-21\", \"status\": \"pending\", \"details\": \"GitHub, AWS, Slack, Jira, Confluence access\"}, {\"task\": \"Training Assignment\", \"assignee\": \"L&D\", \"due\": \"2024-01-22\", \"status\": \"pending\", \"details\": \"Assign Engineering Onboarding curriculum in LMS\"}, {\"task\": \"Team Introductions\", \"assignee\": \"jennifer.walsh@company.com\", \"due\": \"2024-01-24\", \"status\": \"scheduled\", \"details\": \"Calendar invites sent for 1:1s with 5 team leads\"}], \"notifications_sent\": [\"Manager notified\", \"IT ticket created\", \"Welcome email scheduled\"], \"day_1_checklist\": {\"url\": \"https://confluence.internal/onboarding/eng-day1\", \"items\": 12}}",
    "execution_id": "exec-ob-001",
    "conversation_id": "conv-onboard89"
  }
}
```

#### Daily Onboarding Status Check (Cron)

```bash
curl -X POST http://localhost:18791/api/cron \
  -H "Content-Type: application/json" \
  -d '{
    "id": "daily-onboarding-check",
    "name": "Daily Onboarding Status Check",
    "schedule": "0 0 8 * * 1-5",
    "message": "Check status of all active onboardings: 1) Employees starting within 7 days - verify all tasks complete, 2) Employees in first 30 days - check training progress, 3) Any blocked or overdue tasks. Escalate issues immediately.",
    "respond_to": ["slack-hr-ops", "email-hr-director"],
    "enabled": true
  }'
```

---

### 6. Meeting Summarization and Action Item Tracking

Automatically summarize meetings, extract action items, and create follow-up tasks.

**Business Value:**
- 30 minutes saved per meeting in note-taking and distribution
- 100% action item capture rate
- Automatic accountability tracking

#### Trigger: Meeting Recording Complete

```bash
# Meeting platform (Zoom, Teams) triggers this when recording is ready
curl -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "meeting-summarizer",
    "message": "Summarize this meeting:\n\nMeeting: Q1 Planning Session\nDate: 2024-01-15\nDuration: 90 minutes\nAttendees: Sarah (PM), Mike (Eng Lead), Lisa (Design), Tom (QA)\n\nTranscript Summary:\n- Discussed Q1 roadmap priorities\n- Debated feature A vs feature B for sprint 1\n- Decided to prioritize feature A due to customer requests\n- Lisa needs design specs by Friday\n- Mike committed to architecture review by Wednesday\n- Tom raised concerns about test coverage for legacy module\n- Agreed to allocate 2 sprints for tech debt\n- Next sync scheduled for Monday 10am",
    "respond_to": ["slack-team-channel", "jira-connector", "calendar-connector"],
    "metadata": {
      "meeting_id": "MTG-2024-0892",
      "recording_url": "https://zoom.us/rec/xxx",
      "calendar_event_id": "CAL-EVT-123",
      "participants": ["sarah@", "mike@", "lisa@", "tom@"]
    },
    "external_ref": "meeting:MTG-2024-0892"
  }'
```

#### Agent Response

```json
{
  "context": {
    "session_id": "sess-mtg-001",
    "thread_id": null,
    "agent_id": "meeting-summarizer",
    "timestamp": "2024-01-15T16:00:20Z"
  },
  "capability": "respond",
  "payload": {
    "message": "{\"meeting_summary\": {\"title\": \"Q1 Planning Session\", \"date\": \"2024-01-15\", \"duration_minutes\": 90, \"attendees\": 4}, \"executive_summary\": \"Team aligned on Q1 priorities with Feature A as sprint 1 focus. Allocated 2 sprints for tech debt to address legacy test coverage concerns. Clear owners and deadlines assigned for pre-work.\", \"key_decisions\": [{\"decision\": \"Prioritize Feature A over Feature B for Sprint 1\", \"rationale\": \"Higher customer demand\", \"owner\": \"Sarah\"}, {\"decision\": \"Allocate 2 sprints for tech debt\", \"rationale\": \"Address legacy module test coverage\", \"owner\": \"Tom\"}], \"action_items\": [{\"task\": \"Complete design specs for Feature A\", \"owner\": \"Lisa\", \"due\": \"2024-01-19\", \"priority\": \"high\"}, {\"task\": \"Architecture review for Feature A\", \"owner\": \"Mike\", \"due\": \"2024-01-17\", \"priority\": \"high\"}, {\"task\": \"Create tech debt backlog for legacy module\", \"owner\": \"Tom\", \"due\": \"2024-01-22\", \"priority\": \"medium\"}], \"follow_up_meeting\": {\"date\": \"2024-01-22\", \"time\": \"10:00 AM\", \"calendar_invite_sent\": true}, \"parking_lot\": [\"Feature B timing to be revisited in February\"]}",
    "execution_id": "exec-ms-001",
    "conversation_id": "conv-mtg892"
  }
}
```

#### Weekly Meeting Analytics (Cron)

```bash
curl -X POST http://localhost:18791/api/cron \
  -H "Content-Type: application/json" \
  -d '{
    "id": "weekly-meeting-analytics",
    "name": "Weekly Meeting Analytics",
    "schedule": "0 0 9 * * 5",
    "message": "Analyze this week meetings: 1) Total meeting hours per team, 2) Action item completion rate from last week, 3) Recurring meetings with low attendance, 4) Meetings without clear outcomes. Suggest optimizations to reduce meeting load.",
    "respond_to": ["slack-leadership", "email-coo"],
    "enabled": true
  }'
```

---

### 7. Customer Health Monitoring and Churn Prediction

Monitor customer engagement signals, predict churn risk, and trigger proactive outreach.

**Business Value:**
- 40% reduction in customer churn
- Early warning system for at-risk accounts
- Data-driven customer success prioritization

#### Trigger: Daily Customer Health Scan

```bash
curl -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "customer-health-monitor",
    "message": "Analyze customer health for account:\n\nCompany: GlobalTech Industries\nAccount ID: ACCT-5678\nARR: $250,000\nContract End: 2024-06-30\n\nEngagement Metrics (30d):\n- Product logins: 45 (down 60% from prior month)\n- Feature usage: 3 of 12 features active\n- Support tickets: 8 (up 300%)\n- NPS response: 6 (detractor)\n- Champion contact: Left company 2 weeks ago\n\nRecent Support Issues:\n- Performance complaints (3 tickets)\n- Integration failures (2 tickets)\n- Feature requests declined (2 tickets)",
    "respond_to": ["salesforce-update", "slack-cs-alerts", "gainsight-connector"],
    "metadata": {
      "account_id": "ACCT-5678",
      "arr": 250000,
      "contract_end": "2024-06-30",
      "csm": "rachel.kim@company.com",
      "days_to_renewal": 167
    },
    "external_ref": "account:ACCT-5678"
  }'
```

#### Agent Response

```json
{
  "context": {
    "session_id": "sess-health-001",
    "thread_id": null,
    "agent_id": "customer-health-monitor",
    "timestamp": "2024-01-15T07:00:45Z"
  },
  "capability": "respond",
  "payload": {
    "message": "{\"health_score\": 32, \"health_trend\": \"declining\", \"churn_risk\": \"high\", \"risk_probability\": 0.73, \"risk_factors\": [{\"factor\": \"Champion departure\", \"weight\": \"critical\", \"detail\": \"Primary contact left 2 weeks ago, no new champion identified\"}, {\"factor\": \"Engagement drop\", \"weight\": \"high\", \"detail\": \"60% reduction in logins indicates potential disengagement\"}, {\"factor\": \"Support escalation\", \"weight\": \"high\", \"detail\": \"300% increase in tickets suggests product frustration\"}, {\"factor\": \"Low feature adoption\", \"weight\": \"medium\", \"detail\": \"Only using 25% of available features, not seeing full value\"}], \"recommended_actions\": [{\"action\": \"Executive sponsor outreach\", \"priority\": \"immediate\", \"owner\": \"VP Customer Success\", \"detail\": \"Schedule call with GlobalTech VP to identify new champion\"}, {\"action\": \"Technical review\", \"priority\": \"this_week\", \"owner\": \"Solutions Engineer\", \"detail\": \"Address performance and integration issues\"}, {\"action\": \"Value realization session\", \"priority\": \"next_2_weeks\", \"owner\": \"CSM\", \"detail\": \"Demo underutilized features that address their use cases\"}], \"renewal_forecast\": {\"probability\": 0.35, \"at_risk_arr\": 250000, \"recommended_discount\": \"Consider 15% renewal discount with 2-year commitment\"}}",
    "execution_id": "exec-ch-001",
    "conversation_id": "conv-acct5678"
  }
}
```

#### Daily Churn Risk Report (Cron)

```bash
curl -X POST http://localhost:18791/api/cron \
  -H "Content-Type: application/json" \
  -d '{
    "id": "daily-churn-risk-report",
    "name": "Daily Churn Risk Report",
    "schedule": "0 0 7 * * 1-5",
    "message": "Generate daily churn risk report: 1) Accounts with health score drops > 20 points, 2) Accounts approaching renewal with risk score > 50, 3) New detractors from NPS surveys, 4) Accounts with champion changes. Prioritize by ARR impact and days to renewal.",
    "respond_to": ["slack-cs-leadership", "salesforce-dashboard"],
    "enabled": true
  }'
```

---

## Integration Patterns

### Salesforce Integration

```bash
# Register Salesforce connector for CRM updates
curl -X POST http://localhost:18791/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "salesforce-connector",
    "name": "Salesforce CRM Connector",
    "transport": {
      "type": "http",
      "callback_url": "https://your-middleware.com/salesforce/webhook",
      "method": "POST",
      "headers": {
        "Authorization": "Bearer YOUR_SALESFORCE_TOKEN",
        "Content-Type": "application/json"
      },
      "timeout_ms": 30000
    },
    "enabled": true
  }'
```

**Middleware receives webhook and translates to Salesforce API:**
- Update Lead/Contact records
- Create Tasks and Events
- Update Opportunity stages
- Log Activities

### HubSpot Integration

```bash
curl -X POST http://localhost:18791/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "hubspot-connector",
    "name": "HubSpot CRM Connector",
    "transport": {
      "type": "http",
      "callback_url": "https://your-middleware.com/hubspot/webhook",
      "method": "POST",
      "headers": {
        "Authorization": "Bearer YOUR_HUBSPOT_API_KEY",
        "Content-Type": "application/json"
      },
      "timeout_ms": 30000
    },
    "enabled": true
  }'
```

### Zendesk Integration

```bash
curl -X POST http://localhost:18791/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "zendesk-connector",
    "name": "Zendesk Support Connector",
    "transport": {
      "type": "http",
      "callback_url": "https://your-middleware.com/zendesk/webhook",
      "method": "POST",
      "headers": {
        "Authorization": "Basic BASE64_ENCODED_CREDS",
        "Content-Type": "application/json"
      },
      "timeout_ms": 30000
    },
    "enabled": true
  }'
```

### Slack Integration

```bash
curl -X POST http://localhost:18791/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "slack-notifications",
    "name": "Slack Notifications",
    "transport": {
      "type": "http",
      "callback_url": "https://hooks.slack.com/services/T00000000/B00000000/XXXXXXXX",
      "method": "POST",
      "headers": {
        "Content-Type": "application/json"
      },
      "timeout_ms": 10000
    },
    "enabled": true
  }'
```

### Microsoft Teams Integration

```bash
curl -X POST http://localhost:18791/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "teams-notifications",
    "name": "Microsoft Teams Notifications",
    "transport": {
      "type": "http",
      "callback_url": "https://outlook.office.com/webhook/YOUR_WEBHOOK_URL",
      "method": "POST",
      "headers": {
        "Content-Type": "application/json"
      },
      "timeout_ms": 10000
    },
    "enabled": true
  }'
```

---

## Connector Configurations

### Multi-Channel Alert Configuration

For critical business operations, route to multiple channels:

```bash
# Create a high-priority sales alert setup
curl -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "sales-alert-agent",
    "message": "Enterprise deal closing signal detected...",
    "respond_to": [
      "salesforce-connector",
      "slack-sales-urgent",
      "teams-sales-channel",
      "email-sales-vp"
    ],
    "metadata": {
      "alert_type": "deal_signal",
      "priority": "critical"
    }
  }'
```

### CLI Connector for Local Processing

```bash
curl -X POST http://localhost:18791/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "local-processor",
    "name": "Local Data Processor",
    "transport": {
      "type": "cli",
      "command": "/opt/scripts/process-agent-response.sh",
      "args": ["--format", "json", "--output", "/var/data/responses/"],
      "env": {
        "DB_CONNECTION": "postgresql://localhost/analytics"
      }
    },
    "enabled": true
  }'
```

### Testing Connectors

Always test connectors before relying on them in production:

```bash
# Test a connector
curl -X POST http://localhost:18791/api/connectors/salesforce-connector/test

# Check connector status
curl http://localhost:18791/api/connectors/salesforce-connector

# Disable a problematic connector
curl -X POST http://localhost:18791/api/connectors/salesforce-connector/disable
```

---

## ROI and Efficiency Metrics

### Customer Support

| Metric | Before | After | Improvement |
|--------|--------|-------|-------------|
| First Response Time | 4 hours | 15 minutes | 94% faster |
| Ticket Routing Accuracy | 72% | 95% | 32% improvement |
| Tickets/Agent/Day | 25 | 45 | 80% increase |
| Customer Satisfaction | 3.8/5 | 4.4/5 | 16% improvement |

### Sales Operations

| Metric | Before | After | Improvement |
|--------|--------|-------|-------------|
| Lead Qualification Time | 2 days | 2 hours | 95% faster |
| Lead-to-Opportunity Rate | 12% | 18% | 50% improvement |
| CRM Data Accuracy | 65% | 95% | 46% improvement |
| Sales Cycle Length | 45 days | 38 days | 16% faster |

### Finance Operations

| Metric | Before | After | Improvement |
|--------|--------|-------|-------------|
| Invoice Processing Time | 5 days | 1 day | 80% faster |
| Data Entry Errors | 8% | 0.5% | 94% reduction |
| Early Payment Discount Capture | 40% | 85% | 113% improvement |
| AP FTE Required | 5 | 2 | 60% reduction |

### Contract Management

| Metric | Before | After | Improvement |
|--------|--------|-------|-------------|
| Contract Review Time | 5 days | 1 day | 80% faster |
| Compliance Issues Caught | 60% | 95% | 58% improvement |
| Renewal Miss Rate | 12% | 2% | 83% reduction |
| Legal Team Capacity | 100 contracts/mo | 300 contracts/mo | 200% increase |

### Customer Success

| Metric | Before | After | Improvement |
|--------|--------|-------|-------------|
| Churn Rate | 15% | 9% | 40% reduction |
| At-Risk Detection Lead Time | 30 days | 90 days | 200% earlier |
| CSM Account Coverage | 30 accounts | 75 accounts | 150% increase |
| Net Revenue Retention | 95% | 115% | 21% improvement |

---

## Getting Started

### 1. Deploy AgentZero

```bash
# Start the daemon
cargo run -p daemon

# Verify it's running
curl http://localhost:18791/api/health
```

### 2. Create Your First Connector

```bash
curl -X POST http://localhost:18791/api/connectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "my-first-connector",
    "name": "Webhook Test",
    "transport": {
      "type": "http",
      "callback_url": "https://webhook.site/your-unique-url",
      "method": "POST"
    },
    "enabled": true
  }'
```

### 3. Test with a Simple Request

```bash
curl -X POST http://localhost:18791/api/gateway/submit \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "root",
    "message": "Hello, this is a test message!",
    "respond_to": ["my-first-connector"]
  }'
```

### 4. Set Up Your First Scheduled Job

```bash
curl -X POST http://localhost:18791/api/cron \
  -H "Content-Type: application/json" \
  -d '{
    "id": "test-cron",
    "name": "Test Scheduled Job",
    "schedule": "0 */5 * * * *",
    "message": "This is a scheduled test running every 5 minutes",
    "respond_to": ["my-first-connector"],
    "enabled": true
  }'
```

---

## Best Practices

1. **Start Small**: Begin with one use case and expand gradually
2. **Test Connectors**: Always test connectors in a staging environment first
3. **Monitor Responses**: Review agent responses regularly to tune prompts
4. **Handle Failures Gracefully**: Design connectors to handle timeouts and errors
5. **Secure Credentials**: Use environment variables for API keys, never hardcode
6. **Set Appropriate Timeouts**: Balance between reliability and responsiveness
7. **Version Your Agents**: Track changes to agent instructions over time
8. **Log Everything**: Enable detailed logging for debugging and compliance

---

## Support

For questions about business operations integrations:
- Review the [Connector Specification](../../sample_connectors/docs/CONNECTOR_SPEC.md)
- Check the [Quick Start Guide](../../sample_connectors/docs/QUICKSTART.md)
- Explore [Sample Connectors](../../sample_connectors/)
