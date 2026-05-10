export const PROTOCOL_JSON_SCHEMAS = {
  AgentStreamTurnId: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "string",
  "type": "string"
},
  AgentStreamItemId: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "string",
  "type": "string"
},
  AgentToolCallOutcome: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "completed",
    "failed",
    "cancelled"
  ],
  "title": "AgentToolCallOutcome",
  "type": "string"
},
  RuntimeLanePendingState: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "queued",
    "waitingForApproval",
    "waitingForInput"
  ],
  "title": "RuntimeLanePendingState",
  "type": "string"
},
  AgentStreamEvent: {
  "$defs": {
    "AgentStreamFrame": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "assistantTurnStarted",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "delta": {
              "type": "string"
            },
            "kind": {
              "const": "assistantMessageDelta",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "delta"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "assistantTurnCompleted",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "input": {
              "type": "string"
            },
            "kind": {
              "const": "toolCallStarted",
              "type": "string"
            },
            "toolName": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "toolName",
            "input"
          ],
          "type": "object"
        },
        {
          "properties": {
            "delta": {
              "type": "string"
            },
            "kind": {
              "const": "toolCallProgressed",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "delta"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "toolCallCompleted",
              "type": "string"
            },
            "outcome": {
              "$ref": "#/$defs/AgentToolCallOutcome"
            }
          },
          "required": [
            "kind",
            "outcome"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "pendingStateChanged",
              "type": "string"
            },
            "state": {
              "$ref": "#/$defs/RuntimeLanePendingState"
            }
          },
          "required": [
            "kind",
            "state"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "tokenUsageUpdated",
              "type": "string"
            },
            "modelContextWindow": {
              "format": "uint64",
              "minimum": 0,
              "type": [
                "integer",
                "null"
              ]
            },
            "totalTokens": {
              "format": "uint64",
              "minimum": 0,
              "type": [
                "integer",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "AgentToolCallOutcome": {
      "enum": [
        "completed",
        "failed",
        "cancelled"
      ],
      "type": "string"
    },
    "RuntimeLanePendingState": {
      "enum": [
        "queued",
        "waitingForApproval",
        "waitingForInput"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "fragmentSequence": {
      "format": "uint64",
      "minimum": 0,
      "type": [
        "integer",
        "null"
      ]
    },
    "frame": {
      "$ref": "#/$defs/AgentStreamFrame"
    },
    "itemId": {
      "type": [
        "string",
        "null"
      ]
    },
    "runId": {
      "type": "string"
    },
    "turnId": {
      "type": [
        "string",
        "null"
      ]
    }
  },
  "required": [
    "runId",
    "frame"
  ],
  "title": "AgentStreamEvent",
  "type": "object"
},
  AgentStreamFrame: {
  "$defs": {
    "AgentToolCallOutcome": {
      "enum": [
        "completed",
        "failed",
        "cancelled"
      ],
      "type": "string"
    },
    "RuntimeLanePendingState": {
      "enum": [
        "queued",
        "waitingForApproval",
        "waitingForInput"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "oneOf": [
    {
      "properties": {
        "kind": {
          "const": "assistantTurnStarted",
          "type": "string"
        }
      },
      "required": [
        "kind"
      ],
      "type": "object"
    },
    {
      "properties": {
        "delta": {
          "type": "string"
        },
        "kind": {
          "const": "assistantMessageDelta",
          "type": "string"
        }
      },
      "required": [
        "kind",
        "delta"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "assistantTurnCompleted",
          "type": "string"
        }
      },
      "required": [
        "kind"
      ],
      "type": "object"
    },
    {
      "properties": {
        "input": {
          "type": "string"
        },
        "kind": {
          "const": "toolCallStarted",
          "type": "string"
        },
        "toolName": {
          "type": "string"
        }
      },
      "required": [
        "kind",
        "toolName",
        "input"
      ],
      "type": "object"
    },
    {
      "properties": {
        "delta": {
          "type": "string"
        },
        "kind": {
          "const": "toolCallProgressed",
          "type": "string"
        }
      },
      "required": [
        "kind",
        "delta"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "toolCallCompleted",
          "type": "string"
        },
        "outcome": {
          "$ref": "#/$defs/AgentToolCallOutcome"
        }
      },
      "required": [
        "kind",
        "outcome"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "pendingStateChanged",
          "type": "string"
        },
        "state": {
          "$ref": "#/$defs/RuntimeLanePendingState"
        }
      },
      "required": [
        "kind",
        "state"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "tokenUsageUpdated",
          "type": "string"
        },
        "modelContextWindow": {
          "format": "uint64",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "totalTokens": {
          "format": "uint64",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        }
      },
      "required": [
        "kind"
      ],
      "type": "object"
    }
  ],
  "title": "AgentStreamFrame"
},
  BudgetScope: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "run",
    "parentAggregate"
  ],
  "title": "BudgetScope",
  "type": "string"
},
  BudgetMetric: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "tokens",
    "wallClockMs",
    "toolCalls"
  ],
  "title": "BudgetMetric",
  "type": "string"
},
  BudgetBreach: {
  "$defs": {
    "BudgetMetric": {
      "enum": [
        "tokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "string"
    },
    "BudgetScope": {
      "enum": [
        "run",
        "parentAggregate"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "actual": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    },
    "limit": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    },
    "metric": {
      "$ref": "#/$defs/BudgetMetric"
    },
    "scope": {
      "$ref": "#/$defs/BudgetScope"
    }
  },
  "required": [
    "scope",
    "metric",
    "limit",
    "actual"
  ],
  "title": "BudgetBreach",
  "type": "object"
},
  BudgetSnapshot: {
  "$defs": {
    "BudgetScope": {
      "enum": [
        "run",
        "parentAggregate"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "parentRunId": {
      "type": [
        "string",
        "null"
      ]
    },
    "runId": {
      "type": "string"
    },
    "scope": {
      "$ref": "#/$defs/BudgetScope"
    },
    "toolCalls": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    },
    "totalTokens": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    },
    "wallClockMs": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    }
  },
  "required": [
    "runId",
    "scope",
    "totalTokens",
    "wallClockMs",
    "toolCalls"
  ],
  "title": "BudgetSnapshot",
  "type": "object"
},
  BudgetExceededEvent: {
  "$defs": {
    "BudgetBreach": {
      "properties": {
        "actual": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "limit": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "metric": {
          "$ref": "#/$defs/BudgetMetric"
        },
        "scope": {
          "$ref": "#/$defs/BudgetScope"
        }
      },
      "required": [
        "scope",
        "metric",
        "limit",
        "actual"
      ],
      "type": "object"
    },
    "BudgetMetric": {
      "enum": [
        "tokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "string"
    },
    "BudgetScope": {
      "enum": [
        "run",
        "parentAggregate"
      ],
      "type": "string"
    },
    "BudgetSnapshot": {
      "properties": {
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "scope": {
          "$ref": "#/$defs/BudgetScope"
        },
        "toolCalls": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "totalTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "wallClockMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "runId",
        "scope",
        "totalTokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "breach": {
      "$ref": "#/$defs/BudgetBreach"
    },
    "parentRunId": {
      "type": [
        "string",
        "null"
      ]
    },
    "runId": {
      "type": "string"
    },
    "snapshot": {
      "$ref": "#/$defs/BudgetSnapshot"
    }
  },
  "required": [
    "runId",
    "breach",
    "snapshot"
  ],
  "title": "BudgetExceededEvent",
  "type": "object"
},
  BudgetEvent: {
  "$defs": {
    "BudgetBreach": {
      "properties": {
        "actual": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "limit": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "metric": {
          "$ref": "#/$defs/BudgetMetric"
        },
        "scope": {
          "$ref": "#/$defs/BudgetScope"
        }
      },
      "required": [
        "scope",
        "metric",
        "limit",
        "actual"
      ],
      "type": "object"
    },
    "BudgetExceededEvent": {
      "properties": {
        "breach": {
          "$ref": "#/$defs/BudgetBreach"
        },
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "snapshot": {
          "$ref": "#/$defs/BudgetSnapshot"
        }
      },
      "required": [
        "runId",
        "breach",
        "snapshot"
      ],
      "type": "object"
    },
    "BudgetMetric": {
      "enum": [
        "tokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "string"
    },
    "BudgetScope": {
      "enum": [
        "run",
        "parentAggregate"
      ],
      "type": "string"
    },
    "BudgetSnapshot": {
      "properties": {
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "scope": {
          "$ref": "#/$defs/BudgetScope"
        },
        "toolCalls": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "totalTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "wallClockMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "runId",
        "scope",
        "totalTokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "oneOf": [
    {
      "properties": {
        "event": {
          "$ref": "#/$defs/BudgetExceededEvent"
        },
        "phase": {
          "const": "exceeded",
          "type": "string"
        }
      },
      "required": [
        "phase",
        "event"
      ],
      "type": "object"
    }
  ],
  "title": "BudgetEvent"
},
  ApprovalDecision: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "approved",
    "rejected"
  ],
  "title": "ApprovalDecision",
  "type": "string"
},
  ApprovalResolutionReason: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "user",
    "expired",
    "cancelled",
    "budgetExceeded",
    "runtimePolicy"
  ],
  "title": "ApprovalResolutionReason",
  "type": "string"
},
  ApprovalId: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "string",
  "type": "string"
},
  ApprovalRequest: {
  "$defs": {
    "ApprovalScope": {
      "enum": [
        "fileWrite",
        "processExec",
        "networkAccess"
      ],
      "type": "string"
    },
    "ApprovalTarget": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "toolCall",
              "type": "string"
            },
            "toolName": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "toolName"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "fileWrite",
              "type": "string"
            },
            "paths": {
              "items": {
                "type": "string"
              },
              "type": "array"
            }
          },
          "required": [
            "kind",
            "paths"
          ],
          "type": "object"
        },
        {
          "properties": {
            "command": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "processExec",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "host": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "networkAccess",
              "type": "string"
            },
            "protocol": {
              "type": [
                "string",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "childRunId": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "capsuleDispatch",
              "type": "string"
            },
            "workspaceScope": {
              "anyOf": [
                {
                  "$ref": "#/$defs/WorkspaceMode"
                },
                {
                  "type": "null"
                }
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "WorkspaceMode": {
      "enum": [
        "readonly",
        "workspaceWrite",
        "worktreeWrite",
        "repoWriteWithApproval",
        "remoteWorker",
        "containerized",
        "ephemeral"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "expiresAtMs": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    },
    "id": {
      "type": "string"
    },
    "reason": {
      "type": "string"
    },
    "requestedAtMs": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    },
    "runId": {
      "type": "string"
    },
    "scope": {
      "$ref": "#/$defs/ApprovalScope"
    },
    "target": {
      "$ref": "#/$defs/ApprovalTarget"
    },
    "toolCallId": {
      "type": [
        "string",
        "null"
      ]
    }
  },
  "required": [
    "id",
    "runId",
    "scope",
    "requestedAtMs",
    "expiresAtMs",
    "target",
    "reason"
  ],
  "title": "ApprovalRequest",
  "type": "object"
},
  ApprovalTarget: {
  "$defs": {
    "WorkspaceMode": {
      "enum": [
        "readonly",
        "workspaceWrite",
        "worktreeWrite",
        "repoWriteWithApproval",
        "remoteWorker",
        "containerized",
        "ephemeral"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "oneOf": [
    {
      "properties": {
        "kind": {
          "const": "toolCall",
          "type": "string"
        },
        "toolName": {
          "type": "string"
        }
      },
      "required": [
        "kind",
        "toolName"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "fileWrite",
          "type": "string"
        },
        "paths": {
          "items": {
            "type": "string"
          },
          "type": "array"
        }
      },
      "required": [
        "kind",
        "paths"
      ],
      "type": "object"
    },
    {
      "properties": {
        "command": {
          "type": [
            "string",
            "null"
          ]
        },
        "kind": {
          "const": "processExec",
          "type": "string"
        }
      },
      "required": [
        "kind"
      ],
      "type": "object"
    },
    {
      "properties": {
        "host": {
          "type": [
            "string",
            "null"
          ]
        },
        "kind": {
          "const": "networkAccess",
          "type": "string"
        },
        "protocol": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "kind"
      ],
      "type": "object"
    },
    {
      "properties": {
        "childRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "kind": {
          "const": "capsuleDispatch",
          "type": "string"
        },
        "workspaceScope": {
          "anyOf": [
            {
              "$ref": "#/$defs/WorkspaceMode"
            },
            {
              "type": "null"
            }
          ]
        }
      },
      "required": [
        "kind"
      ],
      "type": "object"
    }
  ],
  "title": "ApprovalTarget"
},
  PublicApprovalEvent: {
  "$defs": {
    "ApprovalDecision": {
      "enum": [
        "approved",
        "rejected"
      ],
      "type": "string"
    },
    "ApprovalRequest": {
      "properties": {
        "expiresAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "id": {
          "type": "string"
        },
        "reason": {
          "type": "string"
        },
        "requestedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "runId": {
          "type": "string"
        },
        "scope": {
          "$ref": "#/$defs/ApprovalScope"
        },
        "target": {
          "$ref": "#/$defs/ApprovalTarget"
        },
        "toolCallId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "runId",
        "scope",
        "requestedAtMs",
        "expiresAtMs",
        "target",
        "reason"
      ],
      "type": "object"
    },
    "ApprovalResolutionReason": {
      "enum": [
        "user",
        "expired",
        "cancelled",
        "budgetExceeded",
        "runtimePolicy"
      ],
      "type": "string"
    },
    "ApprovalScope": {
      "enum": [
        "fileWrite",
        "processExec",
        "networkAccess"
      ],
      "type": "string"
    },
    "ApprovalTarget": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "toolCall",
              "type": "string"
            },
            "toolName": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "toolName"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "fileWrite",
              "type": "string"
            },
            "paths": {
              "items": {
                "type": "string"
              },
              "type": "array"
            }
          },
          "required": [
            "kind",
            "paths"
          ],
          "type": "object"
        },
        {
          "properties": {
            "command": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "processExec",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "host": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "networkAccess",
              "type": "string"
            },
            "protocol": {
              "type": [
                "string",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "childRunId": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "capsuleDispatch",
              "type": "string"
            },
            "workspaceScope": {
              "anyOf": [
                {
                  "$ref": "#/$defs/WorkspaceMode"
                },
                {
                  "type": "null"
                }
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "PublicApprovalResolution": {
      "additionalProperties": false,
      "properties": {
        "approvalId": {
          "type": "string"
        },
        "decision": {
          "$ref": "#/$defs/ApprovalDecision"
        },
        "reason": {
          "$ref": "#/$defs/ApprovalResolutionReason"
        },
        "runId": {
          "type": "string"
        },
        "toolCallId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "approvalId",
        "runId",
        "decision",
        "reason"
      ],
      "type": "object"
    },
    "WorkspaceMode": {
      "enum": [
        "readonly",
        "workspaceWrite",
        "worktreeWrite",
        "repoWriteWithApproval",
        "remoteWorker",
        "containerized",
        "ephemeral"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "oneOf": [
    {
      "additionalProperties": false,
      "properties": {
        "phase": {
          "const": "requested",
          "type": "string"
        },
        "request": {
          "$ref": "#/$defs/ApprovalRequest"
        }
      },
      "required": [
        "phase",
        "request"
      ],
      "type": "object"
    },
    {
      "additionalProperties": false,
      "properties": {
        "phase": {
          "const": "resolved",
          "type": "string"
        },
        "resolution": {
          "$ref": "#/$defs/PublicApprovalResolution"
        }
      },
      "required": [
        "phase",
        "resolution"
      ],
      "type": "object"
    }
  ],
  "title": "PublicApprovalEvent"
},
  PublicApprovalResolution: {
  "$defs": {
    "ApprovalDecision": {
      "enum": [
        "approved",
        "rejected"
      ],
      "type": "string"
    },
    "ApprovalResolutionReason": {
      "enum": [
        "user",
        "expired",
        "cancelled",
        "budgetExceeded",
        "runtimePolicy"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "additionalProperties": false,
  "properties": {
    "approvalId": {
      "type": "string"
    },
    "decision": {
      "$ref": "#/$defs/ApprovalDecision"
    },
    "reason": {
      "$ref": "#/$defs/ApprovalResolutionReason"
    },
    "runId": {
      "type": "string"
    },
    "toolCallId": {
      "type": [
        "string",
        "null"
      ]
    }
  },
  "required": [
    "approvalId",
    "runId",
    "decision",
    "reason"
  ],
  "title": "PublicApprovalResolution",
  "type": "object"
},
  ApprovalScope: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "fileWrite",
    "processExec",
    "networkAccess"
  ],
  "title": "ApprovalScope",
  "type": "string"
},
  ActivityCursor: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "description": "Durable paging cursor for `daemon.activity.page`.\n\nThis is session-scoped durable paging only. It is not the live resume cursor\nused by `daemon.subscribe`.",
  "properties": {
    "sequence": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    }
  },
  "required": [
    "sequence"
  ],
  "title": "ActivityCursor",
  "type": "object"
},
  ActivityPageQuery: {
  "$defs": {
    "ActivityCursor": {
      "description": "Durable paging cursor for `daemon.activity.page`.\n\nThis is session-scoped durable paging only. It is not the live resume cursor\nused by `daemon.subscribe`.",
      "properties": {
        "sequence": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "sequence"
      ],
      "type": "object"
    },
    "DaemonEventKind": {
      "enum": [
        "session",
        "run",
        "runReconciledOnStartup",
        "approval",
        "artifact",
        "contextReceipt",
        "agentStream",
        "tokenUsageRecorded",
        "conflict",
        "budget"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "before": {
      "anyOf": [
        {
          "$ref": "#/$defs/ActivityCursor"
        },
        {
          "type": "null"
        }
      ],
      "description": "Durable paging cursor for older activity items from `daemon.activity.page`."
    },
    "kinds": {
      "items": {
        "$ref": "#/$defs/DaemonEventKind"
      },
      "type": "array"
    },
    "limit": {
      "format": "uint32",
      "minimum": 0,
      "type": "integer"
    }
  },
  "required": [
    "limit"
  ],
  "title": "ActivityPageQuery",
  "type": "object"
},
  PublicActivityPageItem: {
  "$defs": {
    "ActivityCursor": {
      "description": "Durable paging cursor for `daemon.activity.page`.\n\nThis is session-scoped durable paging only. It is not the live resume cursor\nused by `daemon.subscribe`.",
      "properties": {
        "sequence": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "sequence"
      ],
      "type": "object"
    },
    "AgentStreamEvent": {
      "properties": {
        "fragmentSequence": {
          "format": "uint64",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "frame": {
          "$ref": "#/$defs/AgentStreamFrame"
        },
        "itemId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "turnId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "runId",
        "frame"
      ],
      "type": "object"
    },
    "AgentStreamFrame": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "assistantTurnStarted",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "delta": {
              "type": "string"
            },
            "kind": {
              "const": "assistantMessageDelta",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "delta"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "assistantTurnCompleted",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "input": {
              "type": "string"
            },
            "kind": {
              "const": "toolCallStarted",
              "type": "string"
            },
            "toolName": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "toolName",
            "input"
          ],
          "type": "object"
        },
        {
          "properties": {
            "delta": {
              "type": "string"
            },
            "kind": {
              "const": "toolCallProgressed",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "delta"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "toolCallCompleted",
              "type": "string"
            },
            "outcome": {
              "$ref": "#/$defs/AgentToolCallOutcome"
            }
          },
          "required": [
            "kind",
            "outcome"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "pendingStateChanged",
              "type": "string"
            },
            "state": {
              "$ref": "#/$defs/RuntimeLanePendingState"
            }
          },
          "required": [
            "kind",
            "state"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "tokenUsageUpdated",
              "type": "string"
            },
            "modelContextWindow": {
              "format": "uint64",
              "minimum": 0,
              "type": [
                "integer",
                "null"
              ]
            },
            "totalTokens": {
              "format": "uint64",
              "minimum": 0,
              "type": [
                "integer",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "AgentToolCallOutcome": {
      "enum": [
        "completed",
        "failed",
        "cancelled"
      ],
      "type": "string"
    },
    "ApprovalDecision": {
      "enum": [
        "approved",
        "rejected"
      ],
      "type": "string"
    },
    "ApprovalRequest": {
      "properties": {
        "expiresAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "id": {
          "type": "string"
        },
        "reason": {
          "type": "string"
        },
        "requestedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "runId": {
          "type": "string"
        },
        "scope": {
          "$ref": "#/$defs/ApprovalScope"
        },
        "target": {
          "$ref": "#/$defs/ApprovalTarget"
        },
        "toolCallId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "runId",
        "scope",
        "requestedAtMs",
        "expiresAtMs",
        "target",
        "reason"
      ],
      "type": "object"
    },
    "ApprovalResolutionReason": {
      "enum": [
        "user",
        "expired",
        "cancelled",
        "budgetExceeded",
        "runtimePolicy"
      ],
      "type": "string"
    },
    "ApprovalScope": {
      "enum": [
        "fileWrite",
        "processExec",
        "networkAccess"
      ],
      "type": "string"
    },
    "ApprovalTarget": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "toolCall",
              "type": "string"
            },
            "toolName": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "toolName"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "fileWrite",
              "type": "string"
            },
            "paths": {
              "items": {
                "type": "string"
              },
              "type": "array"
            }
          },
          "required": [
            "kind",
            "paths"
          ],
          "type": "object"
        },
        {
          "properties": {
            "command": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "processExec",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "host": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "networkAccess",
              "type": "string"
            },
            "protocol": {
              "type": [
                "string",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "childRunId": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "capsuleDispatch",
              "type": "string"
            },
            "workspaceScope": {
              "anyOf": [
                {
                  "$ref": "#/$defs/WorkspaceMode"
                },
                {
                  "type": "null"
                }
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "ArtifactEvent": {
      "properties": {
        "artifact": {
          "$ref": "#/$defs/ArtifactSummary"
        }
      },
      "required": [
        "artifact"
      ],
      "type": "object"
    },
    "ArtifactKind": {
      "enum": [
        "Transcript",
        "Patch",
        "FileSnapshot",
        "CommandLog"
      ],
      "type": "string"
    },
    "ArtifactSummary": {
      "properties": {
        "id": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/$defs/ArtifactKind"
        },
        "runId": {
          "type": "string"
        },
        "storagePath": {
          "type": "string"
        }
      },
      "required": [
        "id",
        "runId",
        "kind",
        "storagePath"
      ],
      "type": "object"
    },
    "BudgetBreach": {
      "properties": {
        "actual": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "limit": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "metric": {
          "$ref": "#/$defs/BudgetMetric"
        },
        "scope": {
          "$ref": "#/$defs/BudgetScope"
        }
      },
      "required": [
        "scope",
        "metric",
        "limit",
        "actual"
      ],
      "type": "object"
    },
    "BudgetEvent": {
      "oneOf": [
        {
          "properties": {
            "event": {
              "$ref": "#/$defs/BudgetExceededEvent"
            },
            "phase": {
              "const": "exceeded",
              "type": "string"
            }
          },
          "required": [
            "phase",
            "event"
          ],
          "type": "object"
        }
      ]
    },
    "BudgetExceededEvent": {
      "properties": {
        "breach": {
          "$ref": "#/$defs/BudgetBreach"
        },
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "snapshot": {
          "$ref": "#/$defs/BudgetSnapshot"
        }
      },
      "required": [
        "runId",
        "breach",
        "snapshot"
      ],
      "type": "object"
    },
    "BudgetMetric": {
      "enum": [
        "tokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "string"
    },
    "BudgetScope": {
      "enum": [
        "run",
        "parentAggregate"
      ],
      "type": "string"
    },
    "BudgetSnapshot": {
      "properties": {
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "scope": {
          "$ref": "#/$defs/BudgetScope"
        },
        "toolCalls": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "totalTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "wallClockMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "runId",
        "scope",
        "totalTokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "object"
    },
    "CapsuleResult": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "debug",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/DebugResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "patch",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/PatchResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "review",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/ReviewResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "test",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/TestResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "plan",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/PlanResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "custom",
              "type": "string"
            },
            "value": true
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        }
      ]
    },
    "ConflictEvent": {
      "oneOf": [
        {
          "properties": {
            "phase": {
              "const": "warning",
              "type": "string"
            },
            "run_id": {
              "type": "string"
            },
            "warning": {
              "$ref": "#/$defs/ConflictWarning"
            }
          },
          "required": [
            "phase",
            "run_id",
            "warning"
          ],
          "type": "object"
        }
      ]
    },
    "ConflictSeverity": {
      "enum": [
        "informational",
        "warning"
      ],
      "type": "string"
    },
    "ConflictWarning": {
      "properties": {
        "conflicts": {
          "items": {
            "$ref": "#/$defs/FileClaimConflict"
          },
          "type": "array"
        },
        "requestingCapsule": {
          "type": "string"
        },
        "severity": {
          "$ref": "#/$defs/ConflictSeverity"
        }
      },
      "required": [
        "requestingCapsule",
        "severity",
        "conflicts"
      ],
      "type": "object"
    },
    "DebugResult": {
      "properties": {
        "blockers": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "confidence": {
          "maximum": 1,
          "minimum": 0,
          "type": "number"
        },
        "evidenceReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "patchReceiptId": {
          "type": [
            "string",
            "null"
          ]
        },
        "reproduced": {
          "type": "boolean"
        },
        "rootCause": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "reproduced",
        "evidenceReceiptIds",
        "confidence",
        "blockers"
      ],
      "type": "object"
    },
    "FileClaimConflict": {
      "properties": {
        "file": {
          "type": "string"
        },
        "holdingCapsule": {
          "type": "string"
        },
        "holdingKind": {
          "$ref": "#/$defs/FileClaimKind"
        }
      },
      "required": [
        "file",
        "holdingCapsule",
        "holdingKind"
      ],
      "type": "object"
    },
    "FileClaimKind": {
      "enum": [
        "write"
      ],
      "type": "string"
    },
    "FindingSeverity": {
      "enum": [
        "low",
        "medium",
        "high",
        "critical"
      ],
      "type": "string"
    },
    "OutputContractKind": {
      "enum": [
        "debug",
        "patch",
        "review",
        "test",
        "plan",
        "custom"
      ],
      "type": "string"
    },
    "PatchResult": {
      "properties": {
        "blockers": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "passing": {
          "type": "boolean"
        },
        "patchReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "testsRunReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "touchedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        }
      },
      "required": [
        "patchReceiptIds",
        "touchedFiles",
        "testsRunReceiptIds",
        "passing",
        "blockers"
      ],
      "type": "object"
    },
    "PlanResult": {
      "properties": {
        "estimatedTotalMinutes": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "risks": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "steps": {
          "items": {
            "$ref": "#/$defs/PlanStep"
          },
          "type": "array"
        }
      },
      "required": [
        "steps",
        "risks"
      ],
      "type": "object"
    },
    "PlanStep": {
      "properties": {
        "dependsOn": {
          "items": {
            "format": "uint32",
            "minimum": 0,
            "type": "integer"
          },
          "type": "array"
        },
        "description": {
          "type": [
            "string",
            "null"
          ]
        },
        "estimatedMinutes": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "title": {
          "type": "string"
        }
      },
      "required": [
        "title",
        "dependsOn"
      ],
      "type": "object"
    },
    "PublicApprovalEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "requested",
              "type": "string"
            },
            "request": {
              "$ref": "#/$defs/ApprovalRequest"
            }
          },
          "required": [
            "phase",
            "request"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "resolved",
              "type": "string"
            },
            "resolution": {
              "$ref": "#/$defs/PublicApprovalResolution"
            }
          },
          "required": [
            "phase",
            "resolution"
          ],
          "type": "object"
        }
      ]
    },
    "PublicApprovalResolution": {
      "additionalProperties": false,
      "properties": {
        "approvalId": {
          "type": "string"
        },
        "decision": {
          "$ref": "#/$defs/ApprovalDecision"
        },
        "reason": {
          "$ref": "#/$defs/ApprovalResolutionReason"
        },
        "runId": {
          "type": "string"
        },
        "toolCallId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "approvalId",
        "runId",
        "decision",
        "reason"
      ],
      "type": "object"
    },
    "PublicContextReceipt": {
      "additionalProperties": false,
      "properties": {
        "id": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/$defs/ReceiptKind"
        },
        "provenance": {
          "$ref": "#/$defs/ReceiptProvenance"
        },
        "state": {
          "$ref": "#/$defs/ReceiptState"
        },
        "summary": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "kind",
        "state",
        "provenance"
      ],
      "type": "object"
    },
    "PublicContextReceiptEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "created",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "promoted",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "quarantined",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        }
      ]
    },
    "PublicDaemonEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "session": {
              "$ref": "#/$defs/SessionEvent"
            }
          },
          "required": [
            "session"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "run": {
              "$ref": "#/$defs/RunEvent"
            }
          },
          "required": [
            "run"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "runReconciledOnStartup": {
              "$ref": "#/$defs/RunReconciledOnStartupEvent"
            }
          },
          "required": [
            "runReconciledOnStartup"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "approval": {
              "$ref": "#/$defs/PublicApprovalEvent"
            }
          },
          "required": [
            "approval"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "artifact": {
              "$ref": "#/$defs/ArtifactEvent"
            }
          },
          "required": [
            "artifact"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "contextReceipt": {
              "$ref": "#/$defs/PublicContextReceiptEvent"
            }
          },
          "required": [
            "contextReceipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "agentStream": {
              "$ref": "#/$defs/AgentStreamEvent"
            }
          },
          "required": [
            "agentStream"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "tokenUsageRecorded": {
              "$ref": "#/$defs/TokenUsageRecordedEvent"
            }
          },
          "required": [
            "tokenUsageRecorded"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "conflict": {
              "$ref": "#/$defs/ConflictEvent"
            }
          },
          "required": [
            "conflict"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "budget": {
              "$ref": "#/$defs/BudgetEvent"
            }
          },
          "required": [
            "budget"
          ],
          "type": "object"
        }
      ]
    },
    "ReceiptKind": {
      "enum": [
        "evidence",
        "patch",
        "testOutput",
        "reviewFinding",
        "artifact",
        "risk",
        "blocker",
        "summary"
      ],
      "type": "string"
    },
    "ReceiptProvenance": {
      "description": "Provenance shape rules:\n- artifact-derived: only `artifact_id` is set; identity = (session, run, kind, artifact_id).\n- event-derived: both `event_seq` and `agent_turn_id` are set; identity = (session, run, kind, event_seq, agent_turn_id).\n- free-form: all identifying fields are None.\n\n`stream_cursor` is descriptive metadata (e.g. for UI navigation) and may be\npresent in any shape. It is never part of the unique identity.",
      "properties": {
        "agentTurnId": {
          "type": [
            "string",
            "null"
          ]
        },
        "artifactId": {
          "type": [
            "string",
            "null"
          ]
        },
        "eventSeq": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "streamCursor": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "type": "object"
    },
    "ReceiptState": {
      "enum": [
        "returned",
        "promoted",
        "quarantined"
      ],
      "type": "string"
    },
    "ReviewFinding": {
      "properties": {
        "file": {
          "type": [
            "string",
            "null"
          ]
        },
        "line": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "message": {
          "type": "string"
        },
        "severity": {
          "$ref": "#/$defs/FindingSeverity"
        },
        "suggestion": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "severity",
        "message"
      ],
      "type": "object"
    },
    "ReviewResult": {
      "properties": {
        "findings": {
          "items": {
            "$ref": "#/$defs/ReviewFinding"
          },
          "type": "array"
        },
        "risks": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "touchedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "verdict": {
          "$ref": "#/$defs/ReviewVerdict"
        }
      },
      "required": [
        "verdict",
        "findings",
        "risks",
        "touchedFiles"
      ],
      "type": "object"
    },
    "ReviewVerdict": {
      "enum": [
        "approve",
        "requestChanges",
        "needsHuman"
      ],
      "type": "string"
    },
    "RunEvent": {
      "properties": {
        "detail": {
          "type": "string"
        },
        "outputContract": {
          "anyOf": [
            {
              "$ref": "#/$defs/OutputContractKind"
            },
            {
              "type": "null"
            }
          ]
        },
        "recipeId": {
          "type": [
            "string",
            "null"
          ]
        },
        "result": {
          "anyOf": [
            {
              "$ref": "#/$defs/CapsuleResult"
            },
            {
              "type": "null"
            }
          ]
        },
        "runId": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/RunStatus"
        }
      },
      "required": [
        "runId",
        "status",
        "detail"
      ],
      "type": "object"
    },
    "RunFailureKind": {
      "enum": [
        "daemonRestartedWhileRunning"
      ],
      "type": "string"
    },
    "RunReconciledOnStartupEvent": {
      "properties": {
        "prevStatus": {
          "$ref": "#/$defs/RunStatus"
        },
        "reason": {
          "$ref": "#/$defs/RunFailureKind"
        },
        "runId": {
          "type": "string"
        }
      },
      "required": [
        "runId",
        "prevStatus",
        "reason"
      ],
      "type": "object"
    },
    "RunStatus": {
      "enum": [
        "queued",
        "running",
        "waitingForApproval",
        "completed",
        "failed",
        "budgetExceeded",
        "cancelled"
      ],
      "type": "string"
    },
    "RuntimeLanePendingState": {
      "enum": [
        "queued",
        "waitingForApproval",
        "waitingForInput"
      ],
      "type": "string"
    },
    "SessionEvent": {
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/SessionStatus"
        }
      },
      "required": [
        "sessionId",
        "status"
      ],
      "type": "object"
    },
    "SessionStatus": {
      "enum": [
        "idle",
        "running",
        "paused",
        "failed",
        "completed"
      ],
      "type": "string"
    },
    "TestResult": {
      "properties": {
        "failed": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "failedTestNames": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "logReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "passed": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "skipped": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "total": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        }
      },
      "required": [
        "total",
        "passed",
        "failed",
        "skipped",
        "failedTestNames",
        "logReceiptIds"
      ],
      "type": "object"
    },
    "TokenUsageRecordedEvent": {
      "properties": {
        "cachedTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "capsuleId": {
          "type": [
            "string",
            "null"
          ]
        },
        "completionTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "model": {
          "type": "string"
        },
        "promptTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "provider": {
          "type": "string"
        },
        "reasoningTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "recordedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "runId": {
          "type": "string"
        }
      },
      "required": [
        "runId",
        "promptTokens",
        "completionTokens",
        "model",
        "provider",
        "recordedAtMs"
      ],
      "type": "object"
    },
    "WorkspaceMode": {
      "enum": [
        "readonly",
        "workspaceWrite",
        "worktreeWrite",
        "repoWriteWithApproval",
        "remoteWorker",
        "containerized",
        "ephemeral"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "additionalProperties": false,
  "properties": {
    "cursor": {
      "$ref": "#/$defs/ActivityCursor"
    },
    "event": {
      "$ref": "#/$defs/PublicDaemonEvent"
    },
    "occurredAtMs": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    }
  },
  "required": [
    "cursor",
    "occurredAtMs",
    "event"
  ],
  "title": "PublicActivityPageItem",
  "type": "object"
},
  PublicActivityPageResult: {
  "$defs": {
    "ActivityCursor": {
      "description": "Durable paging cursor for `daemon.activity.page`.\n\nThis is session-scoped durable paging only. It is not the live resume cursor\nused by `daemon.subscribe`.",
      "properties": {
        "sequence": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "sequence"
      ],
      "type": "object"
    },
    "AgentStreamEvent": {
      "properties": {
        "fragmentSequence": {
          "format": "uint64",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "frame": {
          "$ref": "#/$defs/AgentStreamFrame"
        },
        "itemId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "turnId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "runId",
        "frame"
      ],
      "type": "object"
    },
    "AgentStreamFrame": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "assistantTurnStarted",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "delta": {
              "type": "string"
            },
            "kind": {
              "const": "assistantMessageDelta",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "delta"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "assistantTurnCompleted",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "input": {
              "type": "string"
            },
            "kind": {
              "const": "toolCallStarted",
              "type": "string"
            },
            "toolName": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "toolName",
            "input"
          ],
          "type": "object"
        },
        {
          "properties": {
            "delta": {
              "type": "string"
            },
            "kind": {
              "const": "toolCallProgressed",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "delta"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "toolCallCompleted",
              "type": "string"
            },
            "outcome": {
              "$ref": "#/$defs/AgentToolCallOutcome"
            }
          },
          "required": [
            "kind",
            "outcome"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "pendingStateChanged",
              "type": "string"
            },
            "state": {
              "$ref": "#/$defs/RuntimeLanePendingState"
            }
          },
          "required": [
            "kind",
            "state"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "tokenUsageUpdated",
              "type": "string"
            },
            "modelContextWindow": {
              "format": "uint64",
              "minimum": 0,
              "type": [
                "integer",
                "null"
              ]
            },
            "totalTokens": {
              "format": "uint64",
              "minimum": 0,
              "type": [
                "integer",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "AgentToolCallOutcome": {
      "enum": [
        "completed",
        "failed",
        "cancelled"
      ],
      "type": "string"
    },
    "ApprovalDecision": {
      "enum": [
        "approved",
        "rejected"
      ],
      "type": "string"
    },
    "ApprovalRequest": {
      "properties": {
        "expiresAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "id": {
          "type": "string"
        },
        "reason": {
          "type": "string"
        },
        "requestedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "runId": {
          "type": "string"
        },
        "scope": {
          "$ref": "#/$defs/ApprovalScope"
        },
        "target": {
          "$ref": "#/$defs/ApprovalTarget"
        },
        "toolCallId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "runId",
        "scope",
        "requestedAtMs",
        "expiresAtMs",
        "target",
        "reason"
      ],
      "type": "object"
    },
    "ApprovalResolutionReason": {
      "enum": [
        "user",
        "expired",
        "cancelled",
        "budgetExceeded",
        "runtimePolicy"
      ],
      "type": "string"
    },
    "ApprovalScope": {
      "enum": [
        "fileWrite",
        "processExec",
        "networkAccess"
      ],
      "type": "string"
    },
    "ApprovalTarget": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "toolCall",
              "type": "string"
            },
            "toolName": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "toolName"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "fileWrite",
              "type": "string"
            },
            "paths": {
              "items": {
                "type": "string"
              },
              "type": "array"
            }
          },
          "required": [
            "kind",
            "paths"
          ],
          "type": "object"
        },
        {
          "properties": {
            "command": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "processExec",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "host": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "networkAccess",
              "type": "string"
            },
            "protocol": {
              "type": [
                "string",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "childRunId": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "capsuleDispatch",
              "type": "string"
            },
            "workspaceScope": {
              "anyOf": [
                {
                  "$ref": "#/$defs/WorkspaceMode"
                },
                {
                  "type": "null"
                }
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "ArtifactEvent": {
      "properties": {
        "artifact": {
          "$ref": "#/$defs/ArtifactSummary"
        }
      },
      "required": [
        "artifact"
      ],
      "type": "object"
    },
    "ArtifactKind": {
      "enum": [
        "Transcript",
        "Patch",
        "FileSnapshot",
        "CommandLog"
      ],
      "type": "string"
    },
    "ArtifactSummary": {
      "properties": {
        "id": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/$defs/ArtifactKind"
        },
        "runId": {
          "type": "string"
        },
        "storagePath": {
          "type": "string"
        }
      },
      "required": [
        "id",
        "runId",
        "kind",
        "storagePath"
      ],
      "type": "object"
    },
    "BudgetBreach": {
      "properties": {
        "actual": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "limit": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "metric": {
          "$ref": "#/$defs/BudgetMetric"
        },
        "scope": {
          "$ref": "#/$defs/BudgetScope"
        }
      },
      "required": [
        "scope",
        "metric",
        "limit",
        "actual"
      ],
      "type": "object"
    },
    "BudgetEvent": {
      "oneOf": [
        {
          "properties": {
            "event": {
              "$ref": "#/$defs/BudgetExceededEvent"
            },
            "phase": {
              "const": "exceeded",
              "type": "string"
            }
          },
          "required": [
            "phase",
            "event"
          ],
          "type": "object"
        }
      ]
    },
    "BudgetExceededEvent": {
      "properties": {
        "breach": {
          "$ref": "#/$defs/BudgetBreach"
        },
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "snapshot": {
          "$ref": "#/$defs/BudgetSnapshot"
        }
      },
      "required": [
        "runId",
        "breach",
        "snapshot"
      ],
      "type": "object"
    },
    "BudgetMetric": {
      "enum": [
        "tokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "string"
    },
    "BudgetScope": {
      "enum": [
        "run",
        "parentAggregate"
      ],
      "type": "string"
    },
    "BudgetSnapshot": {
      "properties": {
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "scope": {
          "$ref": "#/$defs/BudgetScope"
        },
        "toolCalls": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "totalTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "wallClockMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "runId",
        "scope",
        "totalTokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "object"
    },
    "CapsuleResult": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "debug",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/DebugResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "patch",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/PatchResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "review",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/ReviewResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "test",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/TestResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "plan",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/PlanResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "custom",
              "type": "string"
            },
            "value": true
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        }
      ]
    },
    "ConflictEvent": {
      "oneOf": [
        {
          "properties": {
            "phase": {
              "const": "warning",
              "type": "string"
            },
            "run_id": {
              "type": "string"
            },
            "warning": {
              "$ref": "#/$defs/ConflictWarning"
            }
          },
          "required": [
            "phase",
            "run_id",
            "warning"
          ],
          "type": "object"
        }
      ]
    },
    "ConflictSeverity": {
      "enum": [
        "informational",
        "warning"
      ],
      "type": "string"
    },
    "ConflictWarning": {
      "properties": {
        "conflicts": {
          "items": {
            "$ref": "#/$defs/FileClaimConflict"
          },
          "type": "array"
        },
        "requestingCapsule": {
          "type": "string"
        },
        "severity": {
          "$ref": "#/$defs/ConflictSeverity"
        }
      },
      "required": [
        "requestingCapsule",
        "severity",
        "conflicts"
      ],
      "type": "object"
    },
    "DebugResult": {
      "properties": {
        "blockers": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "confidence": {
          "maximum": 1,
          "minimum": 0,
          "type": "number"
        },
        "evidenceReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "patchReceiptId": {
          "type": [
            "string",
            "null"
          ]
        },
        "reproduced": {
          "type": "boolean"
        },
        "rootCause": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "reproduced",
        "evidenceReceiptIds",
        "confidence",
        "blockers"
      ],
      "type": "object"
    },
    "FileClaimConflict": {
      "properties": {
        "file": {
          "type": "string"
        },
        "holdingCapsule": {
          "type": "string"
        },
        "holdingKind": {
          "$ref": "#/$defs/FileClaimKind"
        }
      },
      "required": [
        "file",
        "holdingCapsule",
        "holdingKind"
      ],
      "type": "object"
    },
    "FileClaimKind": {
      "enum": [
        "write"
      ],
      "type": "string"
    },
    "FindingSeverity": {
      "enum": [
        "low",
        "medium",
        "high",
        "critical"
      ],
      "type": "string"
    },
    "OutputContractKind": {
      "enum": [
        "debug",
        "patch",
        "review",
        "test",
        "plan",
        "custom"
      ],
      "type": "string"
    },
    "PatchResult": {
      "properties": {
        "blockers": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "passing": {
          "type": "boolean"
        },
        "patchReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "testsRunReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "touchedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        }
      },
      "required": [
        "patchReceiptIds",
        "touchedFiles",
        "testsRunReceiptIds",
        "passing",
        "blockers"
      ],
      "type": "object"
    },
    "PlanResult": {
      "properties": {
        "estimatedTotalMinutes": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "risks": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "steps": {
          "items": {
            "$ref": "#/$defs/PlanStep"
          },
          "type": "array"
        }
      },
      "required": [
        "steps",
        "risks"
      ],
      "type": "object"
    },
    "PlanStep": {
      "properties": {
        "dependsOn": {
          "items": {
            "format": "uint32",
            "minimum": 0,
            "type": "integer"
          },
          "type": "array"
        },
        "description": {
          "type": [
            "string",
            "null"
          ]
        },
        "estimatedMinutes": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "title": {
          "type": "string"
        }
      },
      "required": [
        "title",
        "dependsOn"
      ],
      "type": "object"
    },
    "PublicActivityPageItem": {
      "additionalProperties": false,
      "properties": {
        "cursor": {
          "$ref": "#/$defs/ActivityCursor"
        },
        "event": {
          "$ref": "#/$defs/PublicDaemonEvent"
        },
        "occurredAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "cursor",
        "occurredAtMs",
        "event"
      ],
      "type": "object"
    },
    "PublicApprovalEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "requested",
              "type": "string"
            },
            "request": {
              "$ref": "#/$defs/ApprovalRequest"
            }
          },
          "required": [
            "phase",
            "request"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "resolved",
              "type": "string"
            },
            "resolution": {
              "$ref": "#/$defs/PublicApprovalResolution"
            }
          },
          "required": [
            "phase",
            "resolution"
          ],
          "type": "object"
        }
      ]
    },
    "PublicApprovalResolution": {
      "additionalProperties": false,
      "properties": {
        "approvalId": {
          "type": "string"
        },
        "decision": {
          "$ref": "#/$defs/ApprovalDecision"
        },
        "reason": {
          "$ref": "#/$defs/ApprovalResolutionReason"
        },
        "runId": {
          "type": "string"
        },
        "toolCallId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "approvalId",
        "runId",
        "decision",
        "reason"
      ],
      "type": "object"
    },
    "PublicContextReceipt": {
      "additionalProperties": false,
      "properties": {
        "id": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/$defs/ReceiptKind"
        },
        "provenance": {
          "$ref": "#/$defs/ReceiptProvenance"
        },
        "state": {
          "$ref": "#/$defs/ReceiptState"
        },
        "summary": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "kind",
        "state",
        "provenance"
      ],
      "type": "object"
    },
    "PublicContextReceiptEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "created",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "promoted",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "quarantined",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        }
      ]
    },
    "PublicDaemonEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "session": {
              "$ref": "#/$defs/SessionEvent"
            }
          },
          "required": [
            "session"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "run": {
              "$ref": "#/$defs/RunEvent"
            }
          },
          "required": [
            "run"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "runReconciledOnStartup": {
              "$ref": "#/$defs/RunReconciledOnStartupEvent"
            }
          },
          "required": [
            "runReconciledOnStartup"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "approval": {
              "$ref": "#/$defs/PublicApprovalEvent"
            }
          },
          "required": [
            "approval"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "artifact": {
              "$ref": "#/$defs/ArtifactEvent"
            }
          },
          "required": [
            "artifact"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "contextReceipt": {
              "$ref": "#/$defs/PublicContextReceiptEvent"
            }
          },
          "required": [
            "contextReceipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "agentStream": {
              "$ref": "#/$defs/AgentStreamEvent"
            }
          },
          "required": [
            "agentStream"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "tokenUsageRecorded": {
              "$ref": "#/$defs/TokenUsageRecordedEvent"
            }
          },
          "required": [
            "tokenUsageRecorded"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "conflict": {
              "$ref": "#/$defs/ConflictEvent"
            }
          },
          "required": [
            "conflict"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "budget": {
              "$ref": "#/$defs/BudgetEvent"
            }
          },
          "required": [
            "budget"
          ],
          "type": "object"
        }
      ]
    },
    "ReceiptKind": {
      "enum": [
        "evidence",
        "patch",
        "testOutput",
        "reviewFinding",
        "artifact",
        "risk",
        "blocker",
        "summary"
      ],
      "type": "string"
    },
    "ReceiptProvenance": {
      "description": "Provenance shape rules:\n- artifact-derived: only `artifact_id` is set; identity = (session, run, kind, artifact_id).\n- event-derived: both `event_seq` and `agent_turn_id` are set; identity = (session, run, kind, event_seq, agent_turn_id).\n- free-form: all identifying fields are None.\n\n`stream_cursor` is descriptive metadata (e.g. for UI navigation) and may be\npresent in any shape. It is never part of the unique identity.",
      "properties": {
        "agentTurnId": {
          "type": [
            "string",
            "null"
          ]
        },
        "artifactId": {
          "type": [
            "string",
            "null"
          ]
        },
        "eventSeq": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "streamCursor": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "type": "object"
    },
    "ReceiptState": {
      "enum": [
        "returned",
        "promoted",
        "quarantined"
      ],
      "type": "string"
    },
    "ReviewFinding": {
      "properties": {
        "file": {
          "type": [
            "string",
            "null"
          ]
        },
        "line": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "message": {
          "type": "string"
        },
        "severity": {
          "$ref": "#/$defs/FindingSeverity"
        },
        "suggestion": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "severity",
        "message"
      ],
      "type": "object"
    },
    "ReviewResult": {
      "properties": {
        "findings": {
          "items": {
            "$ref": "#/$defs/ReviewFinding"
          },
          "type": "array"
        },
        "risks": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "touchedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "verdict": {
          "$ref": "#/$defs/ReviewVerdict"
        }
      },
      "required": [
        "verdict",
        "findings",
        "risks",
        "touchedFiles"
      ],
      "type": "object"
    },
    "ReviewVerdict": {
      "enum": [
        "approve",
        "requestChanges",
        "needsHuman"
      ],
      "type": "string"
    },
    "RunEvent": {
      "properties": {
        "detail": {
          "type": "string"
        },
        "outputContract": {
          "anyOf": [
            {
              "$ref": "#/$defs/OutputContractKind"
            },
            {
              "type": "null"
            }
          ]
        },
        "recipeId": {
          "type": [
            "string",
            "null"
          ]
        },
        "result": {
          "anyOf": [
            {
              "$ref": "#/$defs/CapsuleResult"
            },
            {
              "type": "null"
            }
          ]
        },
        "runId": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/RunStatus"
        }
      },
      "required": [
        "runId",
        "status",
        "detail"
      ],
      "type": "object"
    },
    "RunFailureKind": {
      "enum": [
        "daemonRestartedWhileRunning"
      ],
      "type": "string"
    },
    "RunReconciledOnStartupEvent": {
      "properties": {
        "prevStatus": {
          "$ref": "#/$defs/RunStatus"
        },
        "reason": {
          "$ref": "#/$defs/RunFailureKind"
        },
        "runId": {
          "type": "string"
        }
      },
      "required": [
        "runId",
        "prevStatus",
        "reason"
      ],
      "type": "object"
    },
    "RunStatus": {
      "enum": [
        "queued",
        "running",
        "waitingForApproval",
        "completed",
        "failed",
        "budgetExceeded",
        "cancelled"
      ],
      "type": "string"
    },
    "RuntimeLanePendingState": {
      "enum": [
        "queued",
        "waitingForApproval",
        "waitingForInput"
      ],
      "type": "string"
    },
    "SessionEvent": {
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/SessionStatus"
        }
      },
      "required": [
        "sessionId",
        "status"
      ],
      "type": "object"
    },
    "SessionStatus": {
      "enum": [
        "idle",
        "running",
        "paused",
        "failed",
        "completed"
      ],
      "type": "string"
    },
    "TestResult": {
      "properties": {
        "failed": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "failedTestNames": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "logReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "passed": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "skipped": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "total": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        }
      },
      "required": [
        "total",
        "passed",
        "failed",
        "skipped",
        "failedTestNames",
        "logReceiptIds"
      ],
      "type": "object"
    },
    "TokenUsageRecordedEvent": {
      "properties": {
        "cachedTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "capsuleId": {
          "type": [
            "string",
            "null"
          ]
        },
        "completionTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "model": {
          "type": "string"
        },
        "promptTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "provider": {
          "type": "string"
        },
        "reasoningTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "recordedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "runId": {
          "type": "string"
        }
      },
      "required": [
        "runId",
        "promptTokens",
        "completionTokens",
        "model",
        "provider",
        "recordedAtMs"
      ],
      "type": "object"
    },
    "WorkspaceMode": {
      "enum": [
        "readonly",
        "workspaceWrite",
        "worktreeWrite",
        "repoWriteWithApproval",
        "remoteWorker",
        "containerized",
        "ephemeral"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "additionalProperties": false,
  "properties": {
    "items": {
      "items": {
        "$ref": "#/$defs/PublicActivityPageItem"
      },
      "type": "array"
    },
    "latestActivityCursor": {
      "anyOf": [
        {
          "$ref": "#/$defs/ActivityCursor"
        },
        {
          "type": "null"
        }
      ]
    },
    "nextBefore": {
      "anyOf": [
        {
          "$ref": "#/$defs/ActivityCursor"
        },
        {
          "type": "null"
        }
      ]
    }
  },
  "title": "PublicActivityPageResult",
  "type": "object"
},
  AgentTurnsPageQuery: {
  "$defs": {
    "ActivityCursor": {
      "description": "Durable paging cursor for `daemon.activity.page`.\n\nThis is session-scoped durable paging only. It is not the live resume cursor\nused by `daemon.subscribe`.",
      "properties": {
        "sequence": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "sequence"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "before": {
      "anyOf": [
        {
          "$ref": "#/$defs/ActivityCursor"
        },
        {
          "type": "null"
        }
      ]
    },
    "limit": {
      "format": "uint32",
      "minimum": 0,
      "type": "integer"
    }
  },
  "required": [
    "limit"
  ],
  "title": "AgentTurnsPageQuery",
  "type": "object"
},
  AgentAssistantRow: {
  "$defs": {
    "ActivityCursor": {
      "description": "Durable paging cursor for `daemon.activity.page`.\n\nThis is session-scoped durable paging only. It is not the live resume cursor\nused by `daemon.subscribe`.",
      "properties": {
        "sequence": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "sequence"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "completedAtMs": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    },
    "cursor": {
      "$ref": "#/$defs/ActivityCursor"
    },
    "runId": {
      "type": "string"
    },
    "sessionId": {
      "type": "string"
    },
    "startedAtMs": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    },
    "text": {
      "type": "string"
    },
    "turnId": {
      "type": [
        "string",
        "null"
      ]
    }
  },
  "required": [
    "cursor",
    "sessionId",
    "runId",
    "startedAtMs",
    "completedAtMs",
    "text"
  ],
  "title": "AgentAssistantRow",
  "type": "object"
},
  AgentToolCallRow: {
  "$defs": {
    "ActivityCursor": {
      "description": "Durable paging cursor for `daemon.activity.page`.\n\nThis is session-scoped durable paging only. It is not the live resume cursor\nused by `daemon.subscribe`.",
      "properties": {
        "sequence": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "sequence"
      ],
      "type": "object"
    },
    "AgentToolCallOutcome": {
      "enum": [
        "completed",
        "failed",
        "cancelled"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "completedAtMs": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    },
    "cursor": {
      "$ref": "#/$defs/ActivityCursor"
    },
    "input": {
      "type": "string"
    },
    "itemId": {
      "type": [
        "string",
        "null"
      ]
    },
    "outcome": {
      "$ref": "#/$defs/AgentToolCallOutcome"
    },
    "output": {
      "type": "string"
    },
    "runId": {
      "type": "string"
    },
    "sessionId": {
      "type": "string"
    },
    "startedAtMs": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    },
    "toolName": {
      "type": "string"
    },
    "turnId": {
      "type": [
        "string",
        "null"
      ]
    }
  },
  "required": [
    "cursor",
    "sessionId",
    "runId",
    "toolName",
    "input",
    "output",
    "outcome",
    "startedAtMs",
    "completedAtMs"
  ],
  "title": "AgentToolCallRow",
  "type": "object"
},
  AgentPendingStateRow: {
  "$defs": {
    "ActivityCursor": {
      "description": "Durable paging cursor for `daemon.activity.page`.\n\nThis is session-scoped durable paging only. It is not the live resume cursor\nused by `daemon.subscribe`.",
      "properties": {
        "sequence": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "sequence"
      ],
      "type": "object"
    },
    "RuntimeLanePendingState": {
      "enum": [
        "queued",
        "waitingForApproval",
        "waitingForInput"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "cursor": {
      "$ref": "#/$defs/ActivityCursor"
    },
    "occurredAtMs": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    },
    "runId": {
      "type": "string"
    },
    "sessionId": {
      "type": "string"
    },
    "state": {
      "$ref": "#/$defs/RuntimeLanePendingState"
    },
    "turnId": {
      "type": [
        "string",
        "null"
      ]
    }
  },
  "required": [
    "cursor",
    "sessionId",
    "runId",
    "occurredAtMs",
    "state"
  ],
  "title": "AgentPendingStateRow",
  "type": "object"
},
  AgentTurnRow: {
  "$defs": {
    "ActivityCursor": {
      "description": "Durable paging cursor for `daemon.activity.page`.\n\nThis is session-scoped durable paging only. It is not the live resume cursor\nused by `daemon.subscribe`.",
      "properties": {
        "sequence": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "sequence"
      ],
      "type": "object"
    },
    "AgentAssistantRow": {
      "properties": {
        "completedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "cursor": {
          "$ref": "#/$defs/ActivityCursor"
        },
        "runId": {
          "type": "string"
        },
        "sessionId": {
          "type": "string"
        },
        "startedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "text": {
          "type": "string"
        },
        "turnId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "cursor",
        "sessionId",
        "runId",
        "startedAtMs",
        "completedAtMs",
        "text"
      ],
      "type": "object"
    },
    "AgentPendingStateRow": {
      "properties": {
        "cursor": {
          "$ref": "#/$defs/ActivityCursor"
        },
        "occurredAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "runId": {
          "type": "string"
        },
        "sessionId": {
          "type": "string"
        },
        "state": {
          "$ref": "#/$defs/RuntimeLanePendingState"
        },
        "turnId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "cursor",
        "sessionId",
        "runId",
        "occurredAtMs",
        "state"
      ],
      "type": "object"
    },
    "AgentToolCallOutcome": {
      "enum": [
        "completed",
        "failed",
        "cancelled"
      ],
      "type": "string"
    },
    "AgentToolCallRow": {
      "properties": {
        "completedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "cursor": {
          "$ref": "#/$defs/ActivityCursor"
        },
        "input": {
          "type": "string"
        },
        "itemId": {
          "type": [
            "string",
            "null"
          ]
        },
        "outcome": {
          "$ref": "#/$defs/AgentToolCallOutcome"
        },
        "output": {
          "type": "string"
        },
        "runId": {
          "type": "string"
        },
        "sessionId": {
          "type": "string"
        },
        "startedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "toolName": {
          "type": "string"
        },
        "turnId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "cursor",
        "sessionId",
        "runId",
        "toolName",
        "input",
        "output",
        "outcome",
        "startedAtMs",
        "completedAtMs"
      ],
      "type": "object"
    },
    "RuntimeLanePendingState": {
      "enum": [
        "queued",
        "waitingForApproval",
        "waitingForInput"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "oneOf": [
    {
      "$ref": "#/$defs/AgentAssistantRow",
      "properties": {
        "kind": {
          "const": "assistant",
          "type": "string"
        }
      },
      "required": [
        "kind"
      ],
      "type": "object"
    },
    {
      "$ref": "#/$defs/AgentToolCallRow",
      "properties": {
        "kind": {
          "const": "toolCall",
          "type": "string"
        }
      },
      "required": [
        "kind"
      ],
      "type": "object"
    },
    {
      "$ref": "#/$defs/AgentPendingStateRow",
      "properties": {
        "kind": {
          "const": "pendingState",
          "type": "string"
        }
      },
      "required": [
        "kind"
      ],
      "type": "object"
    }
  ],
  "title": "AgentTurnRow"
},
  AgentTurnsPageResult: {
  "$defs": {
    "ActivityCursor": {
      "description": "Durable paging cursor for `daemon.activity.page`.\n\nThis is session-scoped durable paging only. It is not the live resume cursor\nused by `daemon.subscribe`.",
      "properties": {
        "sequence": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "sequence"
      ],
      "type": "object"
    },
    "AgentAssistantRow": {
      "properties": {
        "completedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "cursor": {
          "$ref": "#/$defs/ActivityCursor"
        },
        "runId": {
          "type": "string"
        },
        "sessionId": {
          "type": "string"
        },
        "startedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "text": {
          "type": "string"
        },
        "turnId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "cursor",
        "sessionId",
        "runId",
        "startedAtMs",
        "completedAtMs",
        "text"
      ],
      "type": "object"
    },
    "AgentPendingStateRow": {
      "properties": {
        "cursor": {
          "$ref": "#/$defs/ActivityCursor"
        },
        "occurredAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "runId": {
          "type": "string"
        },
        "sessionId": {
          "type": "string"
        },
        "state": {
          "$ref": "#/$defs/RuntimeLanePendingState"
        },
        "turnId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "cursor",
        "sessionId",
        "runId",
        "occurredAtMs",
        "state"
      ],
      "type": "object"
    },
    "AgentToolCallOutcome": {
      "enum": [
        "completed",
        "failed",
        "cancelled"
      ],
      "type": "string"
    },
    "AgentToolCallRow": {
      "properties": {
        "completedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "cursor": {
          "$ref": "#/$defs/ActivityCursor"
        },
        "input": {
          "type": "string"
        },
        "itemId": {
          "type": [
            "string",
            "null"
          ]
        },
        "outcome": {
          "$ref": "#/$defs/AgentToolCallOutcome"
        },
        "output": {
          "type": "string"
        },
        "runId": {
          "type": "string"
        },
        "sessionId": {
          "type": "string"
        },
        "startedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "toolName": {
          "type": "string"
        },
        "turnId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "cursor",
        "sessionId",
        "runId",
        "toolName",
        "input",
        "output",
        "outcome",
        "startedAtMs",
        "completedAtMs"
      ],
      "type": "object"
    },
    "AgentTurnRow": {
      "oneOf": [
        {
          "$ref": "#/$defs/AgentAssistantRow",
          "properties": {
            "kind": {
              "const": "assistant",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "$ref": "#/$defs/AgentToolCallRow",
          "properties": {
            "kind": {
              "const": "toolCall",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "$ref": "#/$defs/AgentPendingStateRow",
          "properties": {
            "kind": {
              "const": "pendingState",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "DaemonEventCursor": {
      "description": "Resume cursor for `daemon.subscribe` and the `latestCursor` returned from\n`daemon.session.open` / `daemon.session.attach`.\n\nThis cursor is daemon-epoch-aware and scoped to one attached session.",
      "properties": {
        "daemonInstanceId": {
          "type": "string"
        },
        "sequence": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "sessionId": {
          "type": "string"
        }
      },
      "required": [
        "daemonInstanceId",
        "sessionId",
        "sequence"
      ],
      "type": "object"
    },
    "RuntimeLanePendingState": {
      "enum": [
        "queued",
        "waitingForApproval",
        "waitingForInput"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "items": {
      "items": {
        "$ref": "#/$defs/AgentTurnRow"
      },
      "type": "array"
    },
    "latestCursor": {
      "anyOf": [
        {
          "$ref": "#/$defs/DaemonEventCursor"
        },
        {
          "type": "null"
        }
      ]
    },
    "nextBefore": {
      "anyOf": [
        {
          "$ref": "#/$defs/ActivityCursor"
        },
        {
          "type": "null"
        }
      ]
    }
  },
  "title": "AgentTurnsPageResult",
  "type": "object"
},
  ArtifactId: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "string",
  "type": "string"
},
  ArtifactKind: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "Transcript",
    "Patch",
    "FileSnapshot",
    "CommandLog"
  ],
  "title": "ArtifactKind",
  "type": "string"
},
  ArtifactEvent: {
  "$defs": {
    "ArtifactKind": {
      "enum": [
        "Transcript",
        "Patch",
        "FileSnapshot",
        "CommandLog"
      ],
      "type": "string"
    },
    "ArtifactSummary": {
      "properties": {
        "id": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/$defs/ArtifactKind"
        },
        "runId": {
          "type": "string"
        },
        "storagePath": {
          "type": "string"
        }
      },
      "required": [
        "id",
        "runId",
        "kind",
        "storagePath"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "artifact": {
      "$ref": "#/$defs/ArtifactSummary"
    }
  },
  "required": [
    "artifact"
  ],
  "title": "ArtifactEvent",
  "type": "object"
},
  ArtifactSummary: {
  "$defs": {
    "ArtifactKind": {
      "enum": [
        "Transcript",
        "Patch",
        "FileSnapshot",
        "CommandLog"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "id": {
      "type": "string"
    },
    "kind": {
      "$ref": "#/$defs/ArtifactKind"
    },
    "runId": {
      "type": "string"
    },
    "storagePath": {
      "type": "string"
    }
  },
  "required": [
    "id",
    "runId",
    "kind",
    "storagePath"
  ],
  "title": "ArtifactSummary",
  "type": "object"
},
  DaemonClientCapabilities: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "eventSubscriptions": {
      "type": "boolean"
    },
    "notifications": {
      "type": "boolean"
    }
  },
  "required": [
    "notifications",
    "eventSubscriptions"
  ],
  "title": "DaemonClientCapabilities",
  "type": "object"
},
  DaemonEventCursor: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "description": "Resume cursor for `daemon.subscribe` and the `latestCursor` returned from\n`daemon.session.open` / `daemon.session.attach`.\n\nThis cursor is daemon-epoch-aware and scoped to one attached session.",
  "properties": {
    "daemonInstanceId": {
      "type": "string"
    },
    "sequence": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    },
    "sessionId": {
      "type": "string"
    }
  },
  "required": [
    "daemonInstanceId",
    "sessionId",
    "sequence"
  ],
  "title": "DaemonEventCursor",
  "type": "object"
},
  ContextReceipt: {
  "$defs": {
    "ReceiptKind": {
      "enum": [
        "evidence",
        "patch",
        "testOutput",
        "reviewFinding",
        "artifact",
        "risk",
        "blocker",
        "summary"
      ],
      "type": "string"
    },
    "ReceiptProvenance": {
      "description": "Provenance shape rules:\n- artifact-derived: only `artifact_id` is set; identity = (session, run, kind, artifact_id).\n- event-derived: both `event_seq` and `agent_turn_id` are set; identity = (session, run, kind, event_seq, agent_turn_id).\n- free-form: all identifying fields are None.\n\n`stream_cursor` is descriptive metadata (e.g. for UI navigation) and may be\npresent in any shape. It is never part of the unique identity.",
      "properties": {
        "agentTurnId": {
          "type": [
            "string",
            "null"
          ]
        },
        "artifactId": {
          "type": [
            "string",
            "null"
          ]
        },
        "eventSeq": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "streamCursor": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "type": "object"
    },
    "ReceiptState": {
      "enum": [
        "returned",
        "promoted",
        "quarantined"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "createdAtMs": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    },
    "id": {
      "type": "string"
    },
    "kind": {
      "$ref": "#/$defs/ReceiptKind"
    },
    "parentRunId": {
      "type": [
        "string",
        "null"
      ]
    },
    "promotedAtMs": {
      "anyOf": [
        {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        {
          "type": "null"
        }
      ]
    },
    "provenance": {
      "$ref": "#/$defs/ReceiptProvenance"
    },
    "quarantinedAtMs": {
      "anyOf": [
        {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        {
          "type": "null"
        }
      ]
    },
    "runId": {
      "type": "string"
    },
    "sessionId": {
      "type": "string"
    },
    "state": {
      "$ref": "#/$defs/ReceiptState"
    },
    "summary": {
      "type": [
        "string",
        "null"
      ]
    },
    "title": {
      "type": [
        "string",
        "null"
      ]
    }
  },
  "required": [
    "id",
    "sessionId",
    "runId",
    "kind",
    "provenance",
    "state",
    "createdAtMs"
  ],
  "title": "ContextReceipt",
  "type": "object"
},
  ContextReceiptEvent: {
  "$defs": {
    "ContextReceipt": {
      "properties": {
        "createdAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "id": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/$defs/ReceiptKind"
        },
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "promotedAtMs": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "provenance": {
          "$ref": "#/$defs/ReceiptProvenance"
        },
        "quarantinedAtMs": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "runId": {
          "type": "string"
        },
        "sessionId": {
          "type": "string"
        },
        "state": {
          "$ref": "#/$defs/ReceiptState"
        },
        "summary": {
          "type": [
            "string",
            "null"
          ]
        },
        "title": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "sessionId",
        "runId",
        "kind",
        "provenance",
        "state",
        "createdAtMs"
      ],
      "type": "object"
    },
    "ReceiptKind": {
      "enum": [
        "evidence",
        "patch",
        "testOutput",
        "reviewFinding",
        "artifact",
        "risk",
        "blocker",
        "summary"
      ],
      "type": "string"
    },
    "ReceiptProvenance": {
      "description": "Provenance shape rules:\n- artifact-derived: only `artifact_id` is set; identity = (session, run, kind, artifact_id).\n- event-derived: both `event_seq` and `agent_turn_id` are set; identity = (session, run, kind, event_seq, agent_turn_id).\n- free-form: all identifying fields are None.\n\n`stream_cursor` is descriptive metadata (e.g. for UI navigation) and may be\npresent in any shape. It is never part of the unique identity.",
      "properties": {
        "agentTurnId": {
          "type": [
            "string",
            "null"
          ]
        },
        "artifactId": {
          "type": [
            "string",
            "null"
          ]
        },
        "eventSeq": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "streamCursor": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "type": "object"
    },
    "ReceiptState": {
      "enum": [
        "returned",
        "promoted",
        "quarantined"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "oneOf": [
    {
      "properties": {
        "phase": {
          "const": "created",
          "type": "string"
        },
        "receipt": {
          "$ref": "#/$defs/ContextReceipt"
        }
      },
      "required": [
        "phase",
        "receipt"
      ],
      "type": "object"
    },
    {
      "properties": {
        "phase": {
          "const": "promoted",
          "type": "string"
        },
        "receipt": {
          "$ref": "#/$defs/ContextReceipt"
        }
      },
      "required": [
        "phase",
        "receipt"
      ],
      "type": "object"
    },
    {
      "properties": {
        "phase": {
          "const": "quarantined",
          "type": "string"
        },
        "receipt": {
          "$ref": "#/$defs/ContextReceipt"
        }
      },
      "required": [
        "phase",
        "receipt"
      ],
      "type": "object"
    }
  ],
  "title": "ContextReceiptEvent"
},
  PublicContextReceipt: {
  "$defs": {
    "ReceiptKind": {
      "enum": [
        "evidence",
        "patch",
        "testOutput",
        "reviewFinding",
        "artifact",
        "risk",
        "blocker",
        "summary"
      ],
      "type": "string"
    },
    "ReceiptProvenance": {
      "description": "Provenance shape rules:\n- artifact-derived: only `artifact_id` is set; identity = (session, run, kind, artifact_id).\n- event-derived: both `event_seq` and `agent_turn_id` are set; identity = (session, run, kind, event_seq, agent_turn_id).\n- free-form: all identifying fields are None.\n\n`stream_cursor` is descriptive metadata (e.g. for UI navigation) and may be\npresent in any shape. It is never part of the unique identity.",
      "properties": {
        "agentTurnId": {
          "type": [
            "string",
            "null"
          ]
        },
        "artifactId": {
          "type": [
            "string",
            "null"
          ]
        },
        "eventSeq": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "streamCursor": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "type": "object"
    },
    "ReceiptState": {
      "enum": [
        "returned",
        "promoted",
        "quarantined"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "additionalProperties": false,
  "properties": {
    "id": {
      "type": "string"
    },
    "kind": {
      "$ref": "#/$defs/ReceiptKind"
    },
    "provenance": {
      "$ref": "#/$defs/ReceiptProvenance"
    },
    "state": {
      "$ref": "#/$defs/ReceiptState"
    },
    "summary": {
      "type": [
        "string",
        "null"
      ]
    }
  },
  "required": [
    "id",
    "kind",
    "state",
    "provenance"
  ],
  "title": "PublicContextReceipt",
  "type": "object"
},
  PublicContextReceiptEvent: {
  "$defs": {
    "PublicContextReceipt": {
      "additionalProperties": false,
      "properties": {
        "id": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/$defs/ReceiptKind"
        },
        "provenance": {
          "$ref": "#/$defs/ReceiptProvenance"
        },
        "state": {
          "$ref": "#/$defs/ReceiptState"
        },
        "summary": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "kind",
        "state",
        "provenance"
      ],
      "type": "object"
    },
    "ReceiptKind": {
      "enum": [
        "evidence",
        "patch",
        "testOutput",
        "reviewFinding",
        "artifact",
        "risk",
        "blocker",
        "summary"
      ],
      "type": "string"
    },
    "ReceiptProvenance": {
      "description": "Provenance shape rules:\n- artifact-derived: only `artifact_id` is set; identity = (session, run, kind, artifact_id).\n- event-derived: both `event_seq` and `agent_turn_id` are set; identity = (session, run, kind, event_seq, agent_turn_id).\n- free-form: all identifying fields are None.\n\n`stream_cursor` is descriptive metadata (e.g. for UI navigation) and may be\npresent in any shape. It is never part of the unique identity.",
      "properties": {
        "agentTurnId": {
          "type": [
            "string",
            "null"
          ]
        },
        "artifactId": {
          "type": [
            "string",
            "null"
          ]
        },
        "eventSeq": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "streamCursor": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "type": "object"
    },
    "ReceiptState": {
      "enum": [
        "returned",
        "promoted",
        "quarantined"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "oneOf": [
    {
      "additionalProperties": false,
      "properties": {
        "phase": {
          "const": "created",
          "type": "string"
        },
        "receipt": {
          "$ref": "#/$defs/PublicContextReceipt"
        }
      },
      "required": [
        "phase",
        "receipt"
      ],
      "type": "object"
    },
    {
      "additionalProperties": false,
      "properties": {
        "phase": {
          "const": "promoted",
          "type": "string"
        },
        "receipt": {
          "$ref": "#/$defs/PublicContextReceipt"
        }
      },
      "required": [
        "phase",
        "receipt"
      ],
      "type": "object"
    },
    {
      "additionalProperties": false,
      "properties": {
        "phase": {
          "const": "quarantined",
          "type": "string"
        },
        "receipt": {
          "$ref": "#/$defs/PublicContextReceipt"
        }
      },
      "required": [
        "phase",
        "receipt"
      ],
      "type": "object"
    }
  ],
  "title": "PublicContextReceiptEvent"
},
  ReceiptKind: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "evidence",
    "patch",
    "testOutput",
    "reviewFinding",
    "artifact",
    "risk",
    "blocker",
    "summary"
  ],
  "title": "ReceiptKind",
  "type": "string"
},
  ReceiptProvenance: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "description": "Provenance shape rules:\n- artifact-derived: only `artifact_id` is set; identity = (session, run, kind, artifact_id).\n- event-derived: both `event_seq` and `agent_turn_id` are set; identity = (session, run, kind, event_seq, agent_turn_id).\n- free-form: all identifying fields are None.\n\n`stream_cursor` is descriptive metadata (e.g. for UI navigation) and may be\npresent in any shape. It is never part of the unique identity.",
  "properties": {
    "agentTurnId": {
      "type": [
        "string",
        "null"
      ]
    },
    "artifactId": {
      "type": [
        "string",
        "null"
      ]
    },
    "eventSeq": {
      "anyOf": [
        {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        {
          "type": "null"
        }
      ]
    },
    "streamCursor": {
      "type": [
        "string",
        "null"
      ]
    }
  },
  "title": "ReceiptProvenance",
  "type": "object"
},
  ReceiptState: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "returned",
    "promoted",
    "quarantined"
  ],
  "title": "ReceiptState",
  "type": "string"
},
  RunFailureKind: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "daemonRestartedWhileRunning"
  ],
  "title": "RunFailureKind",
  "type": "string"
},
  RunReconciledOnStartupEvent: {
  "$defs": {
    "RunFailureKind": {
      "enum": [
        "daemonRestartedWhileRunning"
      ],
      "type": "string"
    },
    "RunStatus": {
      "enum": [
        "queued",
        "running",
        "waitingForApproval",
        "completed",
        "failed",
        "budgetExceeded",
        "cancelled"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "prevStatus": {
      "$ref": "#/$defs/RunStatus"
    },
    "reason": {
      "$ref": "#/$defs/RunFailureKind"
    },
    "runId": {
      "type": "string"
    }
  },
  "required": [
    "runId",
    "prevStatus",
    "reason"
  ],
  "title": "RunReconciledOnStartupEvent",
  "type": "object"
},
  TokenUsageRecordedEvent: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "cachedTokens": {
      "anyOf": [
        {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        {
          "type": "null"
        }
      ]
    },
    "capsuleId": {
      "type": [
        "string",
        "null"
      ]
    },
    "completionTokens": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    },
    "model": {
      "type": "string"
    },
    "promptTokens": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    },
    "provider": {
      "type": "string"
    },
    "reasoningTokens": {
      "anyOf": [
        {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        {
          "type": "null"
        }
      ]
    },
    "recordedAtMs": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    },
    "runId": {
      "type": "string"
    }
  },
  "required": [
    "runId",
    "promptTokens",
    "completionTokens",
    "model",
    "provider",
    "recordedAtMs"
  ],
  "title": "TokenUsageRecordedEvent",
  "type": "object"
},
  TokenUsageTotals: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "cachedTokens": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    },
    "completionTokens": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    },
    "promptTokens": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    },
    "reasoningTokens": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    }
  },
  "required": [
    "promptTokens",
    "completionTokens",
    "cachedTokens",
    "reasoningTokens"
  ],
  "title": "TokenUsageTotals",
  "type": "object"
},
  PublicDaemonEvent: {
  "$defs": {
    "AgentStreamEvent": {
      "properties": {
        "fragmentSequence": {
          "format": "uint64",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "frame": {
          "$ref": "#/$defs/AgentStreamFrame"
        },
        "itemId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "turnId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "runId",
        "frame"
      ],
      "type": "object"
    },
    "AgentStreamFrame": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "assistantTurnStarted",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "delta": {
              "type": "string"
            },
            "kind": {
              "const": "assistantMessageDelta",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "delta"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "assistantTurnCompleted",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "input": {
              "type": "string"
            },
            "kind": {
              "const": "toolCallStarted",
              "type": "string"
            },
            "toolName": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "toolName",
            "input"
          ],
          "type": "object"
        },
        {
          "properties": {
            "delta": {
              "type": "string"
            },
            "kind": {
              "const": "toolCallProgressed",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "delta"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "toolCallCompleted",
              "type": "string"
            },
            "outcome": {
              "$ref": "#/$defs/AgentToolCallOutcome"
            }
          },
          "required": [
            "kind",
            "outcome"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "pendingStateChanged",
              "type": "string"
            },
            "state": {
              "$ref": "#/$defs/RuntimeLanePendingState"
            }
          },
          "required": [
            "kind",
            "state"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "tokenUsageUpdated",
              "type": "string"
            },
            "modelContextWindow": {
              "format": "uint64",
              "minimum": 0,
              "type": [
                "integer",
                "null"
              ]
            },
            "totalTokens": {
              "format": "uint64",
              "minimum": 0,
              "type": [
                "integer",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "AgentToolCallOutcome": {
      "enum": [
        "completed",
        "failed",
        "cancelled"
      ],
      "type": "string"
    },
    "ApprovalDecision": {
      "enum": [
        "approved",
        "rejected"
      ],
      "type": "string"
    },
    "ApprovalRequest": {
      "properties": {
        "expiresAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "id": {
          "type": "string"
        },
        "reason": {
          "type": "string"
        },
        "requestedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "runId": {
          "type": "string"
        },
        "scope": {
          "$ref": "#/$defs/ApprovalScope"
        },
        "target": {
          "$ref": "#/$defs/ApprovalTarget"
        },
        "toolCallId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "runId",
        "scope",
        "requestedAtMs",
        "expiresAtMs",
        "target",
        "reason"
      ],
      "type": "object"
    },
    "ApprovalResolutionReason": {
      "enum": [
        "user",
        "expired",
        "cancelled",
        "budgetExceeded",
        "runtimePolicy"
      ],
      "type": "string"
    },
    "ApprovalScope": {
      "enum": [
        "fileWrite",
        "processExec",
        "networkAccess"
      ],
      "type": "string"
    },
    "ApprovalTarget": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "toolCall",
              "type": "string"
            },
            "toolName": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "toolName"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "fileWrite",
              "type": "string"
            },
            "paths": {
              "items": {
                "type": "string"
              },
              "type": "array"
            }
          },
          "required": [
            "kind",
            "paths"
          ],
          "type": "object"
        },
        {
          "properties": {
            "command": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "processExec",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "host": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "networkAccess",
              "type": "string"
            },
            "protocol": {
              "type": [
                "string",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "childRunId": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "capsuleDispatch",
              "type": "string"
            },
            "workspaceScope": {
              "anyOf": [
                {
                  "$ref": "#/$defs/WorkspaceMode"
                },
                {
                  "type": "null"
                }
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "ArtifactEvent": {
      "properties": {
        "artifact": {
          "$ref": "#/$defs/ArtifactSummary"
        }
      },
      "required": [
        "artifact"
      ],
      "type": "object"
    },
    "ArtifactKind": {
      "enum": [
        "Transcript",
        "Patch",
        "FileSnapshot",
        "CommandLog"
      ],
      "type": "string"
    },
    "ArtifactSummary": {
      "properties": {
        "id": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/$defs/ArtifactKind"
        },
        "runId": {
          "type": "string"
        },
        "storagePath": {
          "type": "string"
        }
      },
      "required": [
        "id",
        "runId",
        "kind",
        "storagePath"
      ],
      "type": "object"
    },
    "BudgetBreach": {
      "properties": {
        "actual": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "limit": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "metric": {
          "$ref": "#/$defs/BudgetMetric"
        },
        "scope": {
          "$ref": "#/$defs/BudgetScope"
        }
      },
      "required": [
        "scope",
        "metric",
        "limit",
        "actual"
      ],
      "type": "object"
    },
    "BudgetEvent": {
      "oneOf": [
        {
          "properties": {
            "event": {
              "$ref": "#/$defs/BudgetExceededEvent"
            },
            "phase": {
              "const": "exceeded",
              "type": "string"
            }
          },
          "required": [
            "phase",
            "event"
          ],
          "type": "object"
        }
      ]
    },
    "BudgetExceededEvent": {
      "properties": {
        "breach": {
          "$ref": "#/$defs/BudgetBreach"
        },
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "snapshot": {
          "$ref": "#/$defs/BudgetSnapshot"
        }
      },
      "required": [
        "runId",
        "breach",
        "snapshot"
      ],
      "type": "object"
    },
    "BudgetMetric": {
      "enum": [
        "tokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "string"
    },
    "BudgetScope": {
      "enum": [
        "run",
        "parentAggregate"
      ],
      "type": "string"
    },
    "BudgetSnapshot": {
      "properties": {
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "scope": {
          "$ref": "#/$defs/BudgetScope"
        },
        "toolCalls": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "totalTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "wallClockMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "runId",
        "scope",
        "totalTokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "object"
    },
    "CapsuleResult": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "debug",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/DebugResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "patch",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/PatchResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "review",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/ReviewResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "test",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/TestResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "plan",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/PlanResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "custom",
              "type": "string"
            },
            "value": true
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        }
      ]
    },
    "ConflictEvent": {
      "oneOf": [
        {
          "properties": {
            "phase": {
              "const": "warning",
              "type": "string"
            },
            "run_id": {
              "type": "string"
            },
            "warning": {
              "$ref": "#/$defs/ConflictWarning"
            }
          },
          "required": [
            "phase",
            "run_id",
            "warning"
          ],
          "type": "object"
        }
      ]
    },
    "ConflictSeverity": {
      "enum": [
        "informational",
        "warning"
      ],
      "type": "string"
    },
    "ConflictWarning": {
      "properties": {
        "conflicts": {
          "items": {
            "$ref": "#/$defs/FileClaimConflict"
          },
          "type": "array"
        },
        "requestingCapsule": {
          "type": "string"
        },
        "severity": {
          "$ref": "#/$defs/ConflictSeverity"
        }
      },
      "required": [
        "requestingCapsule",
        "severity",
        "conflicts"
      ],
      "type": "object"
    },
    "DebugResult": {
      "properties": {
        "blockers": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "confidence": {
          "maximum": 1,
          "minimum": 0,
          "type": "number"
        },
        "evidenceReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "patchReceiptId": {
          "type": [
            "string",
            "null"
          ]
        },
        "reproduced": {
          "type": "boolean"
        },
        "rootCause": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "reproduced",
        "evidenceReceiptIds",
        "confidence",
        "blockers"
      ],
      "type": "object"
    },
    "FileClaimConflict": {
      "properties": {
        "file": {
          "type": "string"
        },
        "holdingCapsule": {
          "type": "string"
        },
        "holdingKind": {
          "$ref": "#/$defs/FileClaimKind"
        }
      },
      "required": [
        "file",
        "holdingCapsule",
        "holdingKind"
      ],
      "type": "object"
    },
    "FileClaimKind": {
      "enum": [
        "write"
      ],
      "type": "string"
    },
    "FindingSeverity": {
      "enum": [
        "low",
        "medium",
        "high",
        "critical"
      ],
      "type": "string"
    },
    "OutputContractKind": {
      "enum": [
        "debug",
        "patch",
        "review",
        "test",
        "plan",
        "custom"
      ],
      "type": "string"
    },
    "PatchResult": {
      "properties": {
        "blockers": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "passing": {
          "type": "boolean"
        },
        "patchReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "testsRunReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "touchedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        }
      },
      "required": [
        "patchReceiptIds",
        "touchedFiles",
        "testsRunReceiptIds",
        "passing",
        "blockers"
      ],
      "type": "object"
    },
    "PlanResult": {
      "properties": {
        "estimatedTotalMinutes": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "risks": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "steps": {
          "items": {
            "$ref": "#/$defs/PlanStep"
          },
          "type": "array"
        }
      },
      "required": [
        "steps",
        "risks"
      ],
      "type": "object"
    },
    "PlanStep": {
      "properties": {
        "dependsOn": {
          "items": {
            "format": "uint32",
            "minimum": 0,
            "type": "integer"
          },
          "type": "array"
        },
        "description": {
          "type": [
            "string",
            "null"
          ]
        },
        "estimatedMinutes": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "title": {
          "type": "string"
        }
      },
      "required": [
        "title",
        "dependsOn"
      ],
      "type": "object"
    },
    "PublicApprovalEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "requested",
              "type": "string"
            },
            "request": {
              "$ref": "#/$defs/ApprovalRequest"
            }
          },
          "required": [
            "phase",
            "request"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "resolved",
              "type": "string"
            },
            "resolution": {
              "$ref": "#/$defs/PublicApprovalResolution"
            }
          },
          "required": [
            "phase",
            "resolution"
          ],
          "type": "object"
        }
      ]
    },
    "PublicApprovalResolution": {
      "additionalProperties": false,
      "properties": {
        "approvalId": {
          "type": "string"
        },
        "decision": {
          "$ref": "#/$defs/ApprovalDecision"
        },
        "reason": {
          "$ref": "#/$defs/ApprovalResolutionReason"
        },
        "runId": {
          "type": "string"
        },
        "toolCallId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "approvalId",
        "runId",
        "decision",
        "reason"
      ],
      "type": "object"
    },
    "PublicContextReceipt": {
      "additionalProperties": false,
      "properties": {
        "id": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/$defs/ReceiptKind"
        },
        "provenance": {
          "$ref": "#/$defs/ReceiptProvenance"
        },
        "state": {
          "$ref": "#/$defs/ReceiptState"
        },
        "summary": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "kind",
        "state",
        "provenance"
      ],
      "type": "object"
    },
    "PublicContextReceiptEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "created",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "promoted",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "quarantined",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        }
      ]
    },
    "ReceiptKind": {
      "enum": [
        "evidence",
        "patch",
        "testOutput",
        "reviewFinding",
        "artifact",
        "risk",
        "blocker",
        "summary"
      ],
      "type": "string"
    },
    "ReceiptProvenance": {
      "description": "Provenance shape rules:\n- artifact-derived: only `artifact_id` is set; identity = (session, run, kind, artifact_id).\n- event-derived: both `event_seq` and `agent_turn_id` are set; identity = (session, run, kind, event_seq, agent_turn_id).\n- free-form: all identifying fields are None.\n\n`stream_cursor` is descriptive metadata (e.g. for UI navigation) and may be\npresent in any shape. It is never part of the unique identity.",
      "properties": {
        "agentTurnId": {
          "type": [
            "string",
            "null"
          ]
        },
        "artifactId": {
          "type": [
            "string",
            "null"
          ]
        },
        "eventSeq": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "streamCursor": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "type": "object"
    },
    "ReceiptState": {
      "enum": [
        "returned",
        "promoted",
        "quarantined"
      ],
      "type": "string"
    },
    "ReviewFinding": {
      "properties": {
        "file": {
          "type": [
            "string",
            "null"
          ]
        },
        "line": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "message": {
          "type": "string"
        },
        "severity": {
          "$ref": "#/$defs/FindingSeverity"
        },
        "suggestion": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "severity",
        "message"
      ],
      "type": "object"
    },
    "ReviewResult": {
      "properties": {
        "findings": {
          "items": {
            "$ref": "#/$defs/ReviewFinding"
          },
          "type": "array"
        },
        "risks": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "touchedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "verdict": {
          "$ref": "#/$defs/ReviewVerdict"
        }
      },
      "required": [
        "verdict",
        "findings",
        "risks",
        "touchedFiles"
      ],
      "type": "object"
    },
    "ReviewVerdict": {
      "enum": [
        "approve",
        "requestChanges",
        "needsHuman"
      ],
      "type": "string"
    },
    "RunEvent": {
      "properties": {
        "detail": {
          "type": "string"
        },
        "outputContract": {
          "anyOf": [
            {
              "$ref": "#/$defs/OutputContractKind"
            },
            {
              "type": "null"
            }
          ]
        },
        "recipeId": {
          "type": [
            "string",
            "null"
          ]
        },
        "result": {
          "anyOf": [
            {
              "$ref": "#/$defs/CapsuleResult"
            },
            {
              "type": "null"
            }
          ]
        },
        "runId": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/RunStatus"
        }
      },
      "required": [
        "runId",
        "status",
        "detail"
      ],
      "type": "object"
    },
    "RunFailureKind": {
      "enum": [
        "daemonRestartedWhileRunning"
      ],
      "type": "string"
    },
    "RunReconciledOnStartupEvent": {
      "properties": {
        "prevStatus": {
          "$ref": "#/$defs/RunStatus"
        },
        "reason": {
          "$ref": "#/$defs/RunFailureKind"
        },
        "runId": {
          "type": "string"
        }
      },
      "required": [
        "runId",
        "prevStatus",
        "reason"
      ],
      "type": "object"
    },
    "RunStatus": {
      "enum": [
        "queued",
        "running",
        "waitingForApproval",
        "completed",
        "failed",
        "budgetExceeded",
        "cancelled"
      ],
      "type": "string"
    },
    "RuntimeLanePendingState": {
      "enum": [
        "queued",
        "waitingForApproval",
        "waitingForInput"
      ],
      "type": "string"
    },
    "SessionEvent": {
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/SessionStatus"
        }
      },
      "required": [
        "sessionId",
        "status"
      ],
      "type": "object"
    },
    "SessionStatus": {
      "enum": [
        "idle",
        "running",
        "paused",
        "failed",
        "completed"
      ],
      "type": "string"
    },
    "TestResult": {
      "properties": {
        "failed": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "failedTestNames": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "logReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "passed": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "skipped": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "total": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        }
      },
      "required": [
        "total",
        "passed",
        "failed",
        "skipped",
        "failedTestNames",
        "logReceiptIds"
      ],
      "type": "object"
    },
    "TokenUsageRecordedEvent": {
      "properties": {
        "cachedTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "capsuleId": {
          "type": [
            "string",
            "null"
          ]
        },
        "completionTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "model": {
          "type": "string"
        },
        "promptTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "provider": {
          "type": "string"
        },
        "reasoningTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "recordedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "runId": {
          "type": "string"
        }
      },
      "required": [
        "runId",
        "promptTokens",
        "completionTokens",
        "model",
        "provider",
        "recordedAtMs"
      ],
      "type": "object"
    },
    "WorkspaceMode": {
      "enum": [
        "readonly",
        "workspaceWrite",
        "worktreeWrite",
        "repoWriteWithApproval",
        "remoteWorker",
        "containerized",
        "ephemeral"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "oneOf": [
    {
      "additionalProperties": false,
      "properties": {
        "session": {
          "$ref": "#/$defs/SessionEvent"
        }
      },
      "required": [
        "session"
      ],
      "type": "object"
    },
    {
      "additionalProperties": false,
      "properties": {
        "run": {
          "$ref": "#/$defs/RunEvent"
        }
      },
      "required": [
        "run"
      ],
      "type": "object"
    },
    {
      "additionalProperties": false,
      "properties": {
        "runReconciledOnStartup": {
          "$ref": "#/$defs/RunReconciledOnStartupEvent"
        }
      },
      "required": [
        "runReconciledOnStartup"
      ],
      "type": "object"
    },
    {
      "additionalProperties": false,
      "properties": {
        "approval": {
          "$ref": "#/$defs/PublicApprovalEvent"
        }
      },
      "required": [
        "approval"
      ],
      "type": "object"
    },
    {
      "additionalProperties": false,
      "properties": {
        "artifact": {
          "$ref": "#/$defs/ArtifactEvent"
        }
      },
      "required": [
        "artifact"
      ],
      "type": "object"
    },
    {
      "additionalProperties": false,
      "properties": {
        "contextReceipt": {
          "$ref": "#/$defs/PublicContextReceiptEvent"
        }
      },
      "required": [
        "contextReceipt"
      ],
      "type": "object"
    },
    {
      "additionalProperties": false,
      "properties": {
        "agentStream": {
          "$ref": "#/$defs/AgentStreamEvent"
        }
      },
      "required": [
        "agentStream"
      ],
      "type": "object"
    },
    {
      "additionalProperties": false,
      "properties": {
        "tokenUsageRecorded": {
          "$ref": "#/$defs/TokenUsageRecordedEvent"
        }
      },
      "required": [
        "tokenUsageRecorded"
      ],
      "type": "object"
    },
    {
      "additionalProperties": false,
      "properties": {
        "conflict": {
          "$ref": "#/$defs/ConflictEvent"
        }
      },
      "required": [
        "conflict"
      ],
      "type": "object"
    },
    {
      "additionalProperties": false,
      "properties": {
        "budget": {
          "$ref": "#/$defs/BudgetEvent"
        }
      },
      "required": [
        "budget"
      ],
      "type": "object"
    }
  ],
  "title": "PublicDaemonEvent"
},
  PublicDaemonEventEnvelope: {
  "$defs": {
    "AgentStreamEvent": {
      "properties": {
        "fragmentSequence": {
          "format": "uint64",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "frame": {
          "$ref": "#/$defs/AgentStreamFrame"
        },
        "itemId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "turnId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "runId",
        "frame"
      ],
      "type": "object"
    },
    "AgentStreamFrame": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "assistantTurnStarted",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "delta": {
              "type": "string"
            },
            "kind": {
              "const": "assistantMessageDelta",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "delta"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "assistantTurnCompleted",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "input": {
              "type": "string"
            },
            "kind": {
              "const": "toolCallStarted",
              "type": "string"
            },
            "toolName": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "toolName",
            "input"
          ],
          "type": "object"
        },
        {
          "properties": {
            "delta": {
              "type": "string"
            },
            "kind": {
              "const": "toolCallProgressed",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "delta"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "toolCallCompleted",
              "type": "string"
            },
            "outcome": {
              "$ref": "#/$defs/AgentToolCallOutcome"
            }
          },
          "required": [
            "kind",
            "outcome"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "pendingStateChanged",
              "type": "string"
            },
            "state": {
              "$ref": "#/$defs/RuntimeLanePendingState"
            }
          },
          "required": [
            "kind",
            "state"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "tokenUsageUpdated",
              "type": "string"
            },
            "modelContextWindow": {
              "format": "uint64",
              "minimum": 0,
              "type": [
                "integer",
                "null"
              ]
            },
            "totalTokens": {
              "format": "uint64",
              "minimum": 0,
              "type": [
                "integer",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "AgentToolCallOutcome": {
      "enum": [
        "completed",
        "failed",
        "cancelled"
      ],
      "type": "string"
    },
    "ApprovalDecision": {
      "enum": [
        "approved",
        "rejected"
      ],
      "type": "string"
    },
    "ApprovalRequest": {
      "properties": {
        "expiresAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "id": {
          "type": "string"
        },
        "reason": {
          "type": "string"
        },
        "requestedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "runId": {
          "type": "string"
        },
        "scope": {
          "$ref": "#/$defs/ApprovalScope"
        },
        "target": {
          "$ref": "#/$defs/ApprovalTarget"
        },
        "toolCallId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "runId",
        "scope",
        "requestedAtMs",
        "expiresAtMs",
        "target",
        "reason"
      ],
      "type": "object"
    },
    "ApprovalResolutionReason": {
      "enum": [
        "user",
        "expired",
        "cancelled",
        "budgetExceeded",
        "runtimePolicy"
      ],
      "type": "string"
    },
    "ApprovalScope": {
      "enum": [
        "fileWrite",
        "processExec",
        "networkAccess"
      ],
      "type": "string"
    },
    "ApprovalTarget": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "toolCall",
              "type": "string"
            },
            "toolName": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "toolName"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "fileWrite",
              "type": "string"
            },
            "paths": {
              "items": {
                "type": "string"
              },
              "type": "array"
            }
          },
          "required": [
            "kind",
            "paths"
          ],
          "type": "object"
        },
        {
          "properties": {
            "command": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "processExec",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "host": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "networkAccess",
              "type": "string"
            },
            "protocol": {
              "type": [
                "string",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "childRunId": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "capsuleDispatch",
              "type": "string"
            },
            "workspaceScope": {
              "anyOf": [
                {
                  "$ref": "#/$defs/WorkspaceMode"
                },
                {
                  "type": "null"
                }
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "ArtifactEvent": {
      "properties": {
        "artifact": {
          "$ref": "#/$defs/ArtifactSummary"
        }
      },
      "required": [
        "artifact"
      ],
      "type": "object"
    },
    "ArtifactKind": {
      "enum": [
        "Transcript",
        "Patch",
        "FileSnapshot",
        "CommandLog"
      ],
      "type": "string"
    },
    "ArtifactSummary": {
      "properties": {
        "id": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/$defs/ArtifactKind"
        },
        "runId": {
          "type": "string"
        },
        "storagePath": {
          "type": "string"
        }
      },
      "required": [
        "id",
        "runId",
        "kind",
        "storagePath"
      ],
      "type": "object"
    },
    "BudgetBreach": {
      "properties": {
        "actual": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "limit": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "metric": {
          "$ref": "#/$defs/BudgetMetric"
        },
        "scope": {
          "$ref": "#/$defs/BudgetScope"
        }
      },
      "required": [
        "scope",
        "metric",
        "limit",
        "actual"
      ],
      "type": "object"
    },
    "BudgetEvent": {
      "oneOf": [
        {
          "properties": {
            "event": {
              "$ref": "#/$defs/BudgetExceededEvent"
            },
            "phase": {
              "const": "exceeded",
              "type": "string"
            }
          },
          "required": [
            "phase",
            "event"
          ],
          "type": "object"
        }
      ]
    },
    "BudgetExceededEvent": {
      "properties": {
        "breach": {
          "$ref": "#/$defs/BudgetBreach"
        },
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "snapshot": {
          "$ref": "#/$defs/BudgetSnapshot"
        }
      },
      "required": [
        "runId",
        "breach",
        "snapshot"
      ],
      "type": "object"
    },
    "BudgetMetric": {
      "enum": [
        "tokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "string"
    },
    "BudgetScope": {
      "enum": [
        "run",
        "parentAggregate"
      ],
      "type": "string"
    },
    "BudgetSnapshot": {
      "properties": {
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "scope": {
          "$ref": "#/$defs/BudgetScope"
        },
        "toolCalls": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "totalTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "wallClockMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "runId",
        "scope",
        "totalTokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "object"
    },
    "CapsuleResult": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "debug",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/DebugResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "patch",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/PatchResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "review",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/ReviewResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "test",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/TestResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "plan",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/PlanResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "custom",
              "type": "string"
            },
            "value": true
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        }
      ]
    },
    "ConflictEvent": {
      "oneOf": [
        {
          "properties": {
            "phase": {
              "const": "warning",
              "type": "string"
            },
            "run_id": {
              "type": "string"
            },
            "warning": {
              "$ref": "#/$defs/ConflictWarning"
            }
          },
          "required": [
            "phase",
            "run_id",
            "warning"
          ],
          "type": "object"
        }
      ]
    },
    "ConflictSeverity": {
      "enum": [
        "informational",
        "warning"
      ],
      "type": "string"
    },
    "ConflictWarning": {
      "properties": {
        "conflicts": {
          "items": {
            "$ref": "#/$defs/FileClaimConflict"
          },
          "type": "array"
        },
        "requestingCapsule": {
          "type": "string"
        },
        "severity": {
          "$ref": "#/$defs/ConflictSeverity"
        }
      },
      "required": [
        "requestingCapsule",
        "severity",
        "conflicts"
      ],
      "type": "object"
    },
    "DebugResult": {
      "properties": {
        "blockers": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "confidence": {
          "maximum": 1,
          "minimum": 0,
          "type": "number"
        },
        "evidenceReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "patchReceiptId": {
          "type": [
            "string",
            "null"
          ]
        },
        "reproduced": {
          "type": "boolean"
        },
        "rootCause": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "reproduced",
        "evidenceReceiptIds",
        "confidence",
        "blockers"
      ],
      "type": "object"
    },
    "FileClaimConflict": {
      "properties": {
        "file": {
          "type": "string"
        },
        "holdingCapsule": {
          "type": "string"
        },
        "holdingKind": {
          "$ref": "#/$defs/FileClaimKind"
        }
      },
      "required": [
        "file",
        "holdingCapsule",
        "holdingKind"
      ],
      "type": "object"
    },
    "FileClaimKind": {
      "enum": [
        "write"
      ],
      "type": "string"
    },
    "FindingSeverity": {
      "enum": [
        "low",
        "medium",
        "high",
        "critical"
      ],
      "type": "string"
    },
    "OutputContractKind": {
      "enum": [
        "debug",
        "patch",
        "review",
        "test",
        "plan",
        "custom"
      ],
      "type": "string"
    },
    "PatchResult": {
      "properties": {
        "blockers": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "passing": {
          "type": "boolean"
        },
        "patchReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "testsRunReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "touchedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        }
      },
      "required": [
        "patchReceiptIds",
        "touchedFiles",
        "testsRunReceiptIds",
        "passing",
        "blockers"
      ],
      "type": "object"
    },
    "PlanResult": {
      "properties": {
        "estimatedTotalMinutes": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "risks": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "steps": {
          "items": {
            "$ref": "#/$defs/PlanStep"
          },
          "type": "array"
        }
      },
      "required": [
        "steps",
        "risks"
      ],
      "type": "object"
    },
    "PlanStep": {
      "properties": {
        "dependsOn": {
          "items": {
            "format": "uint32",
            "minimum": 0,
            "type": "integer"
          },
          "type": "array"
        },
        "description": {
          "type": [
            "string",
            "null"
          ]
        },
        "estimatedMinutes": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "title": {
          "type": "string"
        }
      },
      "required": [
        "title",
        "dependsOn"
      ],
      "type": "object"
    },
    "PublicApprovalEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "requested",
              "type": "string"
            },
            "request": {
              "$ref": "#/$defs/ApprovalRequest"
            }
          },
          "required": [
            "phase",
            "request"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "resolved",
              "type": "string"
            },
            "resolution": {
              "$ref": "#/$defs/PublicApprovalResolution"
            }
          },
          "required": [
            "phase",
            "resolution"
          ],
          "type": "object"
        }
      ]
    },
    "PublicApprovalResolution": {
      "additionalProperties": false,
      "properties": {
        "approvalId": {
          "type": "string"
        },
        "decision": {
          "$ref": "#/$defs/ApprovalDecision"
        },
        "reason": {
          "$ref": "#/$defs/ApprovalResolutionReason"
        },
        "runId": {
          "type": "string"
        },
        "toolCallId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "approvalId",
        "runId",
        "decision",
        "reason"
      ],
      "type": "object"
    },
    "PublicContextReceipt": {
      "additionalProperties": false,
      "properties": {
        "id": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/$defs/ReceiptKind"
        },
        "provenance": {
          "$ref": "#/$defs/ReceiptProvenance"
        },
        "state": {
          "$ref": "#/$defs/ReceiptState"
        },
        "summary": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "kind",
        "state",
        "provenance"
      ],
      "type": "object"
    },
    "PublicContextReceiptEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "created",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "promoted",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "quarantined",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        }
      ]
    },
    "PublicDaemonEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "session": {
              "$ref": "#/$defs/SessionEvent"
            }
          },
          "required": [
            "session"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "run": {
              "$ref": "#/$defs/RunEvent"
            }
          },
          "required": [
            "run"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "runReconciledOnStartup": {
              "$ref": "#/$defs/RunReconciledOnStartupEvent"
            }
          },
          "required": [
            "runReconciledOnStartup"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "approval": {
              "$ref": "#/$defs/PublicApprovalEvent"
            }
          },
          "required": [
            "approval"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "artifact": {
              "$ref": "#/$defs/ArtifactEvent"
            }
          },
          "required": [
            "artifact"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "contextReceipt": {
              "$ref": "#/$defs/PublicContextReceiptEvent"
            }
          },
          "required": [
            "contextReceipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "agentStream": {
              "$ref": "#/$defs/AgentStreamEvent"
            }
          },
          "required": [
            "agentStream"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "tokenUsageRecorded": {
              "$ref": "#/$defs/TokenUsageRecordedEvent"
            }
          },
          "required": [
            "tokenUsageRecorded"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "conflict": {
              "$ref": "#/$defs/ConflictEvent"
            }
          },
          "required": [
            "conflict"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "budget": {
              "$ref": "#/$defs/BudgetEvent"
            }
          },
          "required": [
            "budget"
          ],
          "type": "object"
        }
      ]
    },
    "ReceiptKind": {
      "enum": [
        "evidence",
        "patch",
        "testOutput",
        "reviewFinding",
        "artifact",
        "risk",
        "blocker",
        "summary"
      ],
      "type": "string"
    },
    "ReceiptProvenance": {
      "description": "Provenance shape rules:\n- artifact-derived: only `artifact_id` is set; identity = (session, run, kind, artifact_id).\n- event-derived: both `event_seq` and `agent_turn_id` are set; identity = (session, run, kind, event_seq, agent_turn_id).\n- free-form: all identifying fields are None.\n\n`stream_cursor` is descriptive metadata (e.g. for UI navigation) and may be\npresent in any shape. It is never part of the unique identity.",
      "properties": {
        "agentTurnId": {
          "type": [
            "string",
            "null"
          ]
        },
        "artifactId": {
          "type": [
            "string",
            "null"
          ]
        },
        "eventSeq": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "streamCursor": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "type": "object"
    },
    "ReceiptState": {
      "enum": [
        "returned",
        "promoted",
        "quarantined"
      ],
      "type": "string"
    },
    "ReviewFinding": {
      "properties": {
        "file": {
          "type": [
            "string",
            "null"
          ]
        },
        "line": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "message": {
          "type": "string"
        },
        "severity": {
          "$ref": "#/$defs/FindingSeverity"
        },
        "suggestion": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "severity",
        "message"
      ],
      "type": "object"
    },
    "ReviewResult": {
      "properties": {
        "findings": {
          "items": {
            "$ref": "#/$defs/ReviewFinding"
          },
          "type": "array"
        },
        "risks": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "touchedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "verdict": {
          "$ref": "#/$defs/ReviewVerdict"
        }
      },
      "required": [
        "verdict",
        "findings",
        "risks",
        "touchedFiles"
      ],
      "type": "object"
    },
    "ReviewVerdict": {
      "enum": [
        "approve",
        "requestChanges",
        "needsHuman"
      ],
      "type": "string"
    },
    "RunEvent": {
      "properties": {
        "detail": {
          "type": "string"
        },
        "outputContract": {
          "anyOf": [
            {
              "$ref": "#/$defs/OutputContractKind"
            },
            {
              "type": "null"
            }
          ]
        },
        "recipeId": {
          "type": [
            "string",
            "null"
          ]
        },
        "result": {
          "anyOf": [
            {
              "$ref": "#/$defs/CapsuleResult"
            },
            {
              "type": "null"
            }
          ]
        },
        "runId": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/RunStatus"
        }
      },
      "required": [
        "runId",
        "status",
        "detail"
      ],
      "type": "object"
    },
    "RunFailureKind": {
      "enum": [
        "daemonRestartedWhileRunning"
      ],
      "type": "string"
    },
    "RunReconciledOnStartupEvent": {
      "properties": {
        "prevStatus": {
          "$ref": "#/$defs/RunStatus"
        },
        "reason": {
          "$ref": "#/$defs/RunFailureKind"
        },
        "runId": {
          "type": "string"
        }
      },
      "required": [
        "runId",
        "prevStatus",
        "reason"
      ],
      "type": "object"
    },
    "RunStatus": {
      "enum": [
        "queued",
        "running",
        "waitingForApproval",
        "completed",
        "failed",
        "budgetExceeded",
        "cancelled"
      ],
      "type": "string"
    },
    "RuntimeLanePendingState": {
      "enum": [
        "queued",
        "waitingForApproval",
        "waitingForInput"
      ],
      "type": "string"
    },
    "SessionEvent": {
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/SessionStatus"
        }
      },
      "required": [
        "sessionId",
        "status"
      ],
      "type": "object"
    },
    "SessionStatus": {
      "enum": [
        "idle",
        "running",
        "paused",
        "failed",
        "completed"
      ],
      "type": "string"
    },
    "TestResult": {
      "properties": {
        "failed": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "failedTestNames": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "logReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "passed": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "skipped": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "total": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        }
      },
      "required": [
        "total",
        "passed",
        "failed",
        "skipped",
        "failedTestNames",
        "logReceiptIds"
      ],
      "type": "object"
    },
    "TokenUsageRecordedEvent": {
      "properties": {
        "cachedTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "capsuleId": {
          "type": [
            "string",
            "null"
          ]
        },
        "completionTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "model": {
          "type": "string"
        },
        "promptTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "provider": {
          "type": "string"
        },
        "reasoningTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "recordedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "runId": {
          "type": "string"
        }
      },
      "required": [
        "runId",
        "promptTokens",
        "completionTokens",
        "model",
        "provider",
        "recordedAtMs"
      ],
      "type": "object"
    },
    "WorkspaceMode": {
      "enum": [
        "readonly",
        "workspaceWrite",
        "worktreeWrite",
        "repoWriteWithApproval",
        "remoteWorker",
        "containerized",
        "ephemeral"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "additionalProperties": false,
  "properties": {
    "daemonInstanceId": {
      "type": "string"
    },
    "event": {
      "$ref": "#/$defs/PublicDaemonEvent"
    },
    "occurredAtMs": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    },
    "sequence": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    },
    "sessionId": {
      "type": "string"
    }
  },
  "required": [
    "daemonInstanceId",
    "sessionId",
    "sequence",
    "occurredAtMs",
    "event"
  ],
  "title": "PublicDaemonEventEnvelope",
  "type": "object"
},
  DaemonEventKind: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "session",
    "run",
    "runReconciledOnStartup",
    "approval",
    "artifact",
    "contextReceipt",
    "agentStream",
    "tokenUsageRecorded",
    "conflict",
    "budget"
  ],
  "title": "DaemonEventKind",
  "type": "string"
},
  SessionOverviewQuery: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "recentActivityLimit": {
      "default": 8,
      "format": "uint32",
      "minimum": 0,
      "type": "integer"
    }
  },
  "title": "SessionOverviewQuery",
  "type": "object"
},
  SessionOverviewResult: {
  "$defs": {
    "AgentStreamEvent": {
      "properties": {
        "fragmentSequence": {
          "format": "uint64",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "frame": {
          "$ref": "#/$defs/AgentStreamFrame"
        },
        "itemId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "turnId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "runId",
        "frame"
      ],
      "type": "object"
    },
    "AgentStreamFrame": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "assistantTurnStarted",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "delta": {
              "type": "string"
            },
            "kind": {
              "const": "assistantMessageDelta",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "delta"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "assistantTurnCompleted",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "input": {
              "type": "string"
            },
            "kind": {
              "const": "toolCallStarted",
              "type": "string"
            },
            "toolName": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "toolName",
            "input"
          ],
          "type": "object"
        },
        {
          "properties": {
            "delta": {
              "type": "string"
            },
            "kind": {
              "const": "toolCallProgressed",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "delta"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "toolCallCompleted",
              "type": "string"
            },
            "outcome": {
              "$ref": "#/$defs/AgentToolCallOutcome"
            }
          },
          "required": [
            "kind",
            "outcome"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "pendingStateChanged",
              "type": "string"
            },
            "state": {
              "$ref": "#/$defs/RuntimeLanePendingState"
            }
          },
          "required": [
            "kind",
            "state"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "tokenUsageUpdated",
              "type": "string"
            },
            "modelContextWindow": {
              "format": "uint64",
              "minimum": 0,
              "type": [
                "integer",
                "null"
              ]
            },
            "totalTokens": {
              "format": "uint64",
              "minimum": 0,
              "type": [
                "integer",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "AgentToolCallOutcome": {
      "enum": [
        "completed",
        "failed",
        "cancelled"
      ],
      "type": "string"
    },
    "ApprovalAttentionState": {
      "enum": [
        "idle",
        "pending"
      ],
      "type": "string"
    },
    "ApprovalDecision": {
      "enum": [
        "approved",
        "rejected"
      ],
      "type": "string"
    },
    "ApprovalRequest": {
      "properties": {
        "expiresAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "id": {
          "type": "string"
        },
        "reason": {
          "type": "string"
        },
        "requestedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "runId": {
          "type": "string"
        },
        "scope": {
          "$ref": "#/$defs/ApprovalScope"
        },
        "target": {
          "$ref": "#/$defs/ApprovalTarget"
        },
        "toolCallId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "runId",
        "scope",
        "requestedAtMs",
        "expiresAtMs",
        "target",
        "reason"
      ],
      "type": "object"
    },
    "ApprovalResolutionReason": {
      "enum": [
        "user",
        "expired",
        "cancelled",
        "budgetExceeded",
        "runtimePolicy"
      ],
      "type": "string"
    },
    "ApprovalScope": {
      "enum": [
        "fileWrite",
        "processExec",
        "networkAccess"
      ],
      "type": "string"
    },
    "ApprovalTarget": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "toolCall",
              "type": "string"
            },
            "toolName": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "toolName"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "fileWrite",
              "type": "string"
            },
            "paths": {
              "items": {
                "type": "string"
              },
              "type": "array"
            }
          },
          "required": [
            "kind",
            "paths"
          ],
          "type": "object"
        },
        {
          "properties": {
            "command": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "processExec",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "host": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "networkAccess",
              "type": "string"
            },
            "protocol": {
              "type": [
                "string",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "childRunId": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "capsuleDispatch",
              "type": "string"
            },
            "workspaceScope": {
              "anyOf": [
                {
                  "$ref": "#/$defs/WorkspaceMode"
                },
                {
                  "type": "null"
                }
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "ArtifactEvent": {
      "properties": {
        "artifact": {
          "$ref": "#/$defs/ArtifactSummary"
        }
      },
      "required": [
        "artifact"
      ],
      "type": "object"
    },
    "ArtifactKind": {
      "enum": [
        "Transcript",
        "Patch",
        "FileSnapshot",
        "CommandLog"
      ],
      "type": "string"
    },
    "ArtifactSummary": {
      "properties": {
        "id": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/$defs/ArtifactKind"
        },
        "runId": {
          "type": "string"
        },
        "storagePath": {
          "type": "string"
        }
      },
      "required": [
        "id",
        "runId",
        "kind",
        "storagePath"
      ],
      "type": "object"
    },
    "BudgetBreach": {
      "properties": {
        "actual": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "limit": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "metric": {
          "$ref": "#/$defs/BudgetMetric"
        },
        "scope": {
          "$ref": "#/$defs/BudgetScope"
        }
      },
      "required": [
        "scope",
        "metric",
        "limit",
        "actual"
      ],
      "type": "object"
    },
    "BudgetEvent": {
      "oneOf": [
        {
          "properties": {
            "event": {
              "$ref": "#/$defs/BudgetExceededEvent"
            },
            "phase": {
              "const": "exceeded",
              "type": "string"
            }
          },
          "required": [
            "phase",
            "event"
          ],
          "type": "object"
        }
      ]
    },
    "BudgetExceededEvent": {
      "properties": {
        "breach": {
          "$ref": "#/$defs/BudgetBreach"
        },
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "snapshot": {
          "$ref": "#/$defs/BudgetSnapshot"
        }
      },
      "required": [
        "runId",
        "breach",
        "snapshot"
      ],
      "type": "object"
    },
    "BudgetMetric": {
      "enum": [
        "tokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "string"
    },
    "BudgetScope": {
      "enum": [
        "run",
        "parentAggregate"
      ],
      "type": "string"
    },
    "BudgetSnapshot": {
      "properties": {
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "scope": {
          "$ref": "#/$defs/BudgetScope"
        },
        "toolCalls": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "totalTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "wallClockMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "runId",
        "scope",
        "totalTokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "object"
    },
    "CapsuleResult": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "debug",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/DebugResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "patch",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/PatchResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "review",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/ReviewResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "test",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/TestResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "plan",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/PlanResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "custom",
              "type": "string"
            },
            "value": true
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        }
      ]
    },
    "ConflictEvent": {
      "oneOf": [
        {
          "properties": {
            "phase": {
              "const": "warning",
              "type": "string"
            },
            "run_id": {
              "type": "string"
            },
            "warning": {
              "$ref": "#/$defs/ConflictWarning"
            }
          },
          "required": [
            "phase",
            "run_id",
            "warning"
          ],
          "type": "object"
        }
      ]
    },
    "ConflictSeverity": {
      "enum": [
        "informational",
        "warning"
      ],
      "type": "string"
    },
    "ConflictWarning": {
      "properties": {
        "conflicts": {
          "items": {
            "$ref": "#/$defs/FileClaimConflict"
          },
          "type": "array"
        },
        "requestingCapsule": {
          "type": "string"
        },
        "severity": {
          "$ref": "#/$defs/ConflictSeverity"
        }
      },
      "required": [
        "requestingCapsule",
        "severity",
        "conflicts"
      ],
      "type": "object"
    },
    "DebugResult": {
      "properties": {
        "blockers": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "confidence": {
          "maximum": 1,
          "minimum": 0,
          "type": "number"
        },
        "evidenceReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "patchReceiptId": {
          "type": [
            "string",
            "null"
          ]
        },
        "reproduced": {
          "type": "boolean"
        },
        "rootCause": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "reproduced",
        "evidenceReceiptIds",
        "confidence",
        "blockers"
      ],
      "type": "object"
    },
    "FileClaimConflict": {
      "properties": {
        "file": {
          "type": "string"
        },
        "holdingCapsule": {
          "type": "string"
        },
        "holdingKind": {
          "$ref": "#/$defs/FileClaimKind"
        }
      },
      "required": [
        "file",
        "holdingCapsule",
        "holdingKind"
      ],
      "type": "object"
    },
    "FileClaimKind": {
      "enum": [
        "write"
      ],
      "type": "string"
    },
    "FindingSeverity": {
      "enum": [
        "low",
        "medium",
        "high",
        "critical"
      ],
      "type": "string"
    },
    "OutputContractKind": {
      "enum": [
        "debug",
        "patch",
        "review",
        "test",
        "plan",
        "custom"
      ],
      "type": "string"
    },
    "PatchResult": {
      "properties": {
        "blockers": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "passing": {
          "type": "boolean"
        },
        "patchReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "testsRunReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "touchedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        }
      },
      "required": [
        "patchReceiptIds",
        "touchedFiles",
        "testsRunReceiptIds",
        "passing",
        "blockers"
      ],
      "type": "object"
    },
    "PlanResult": {
      "properties": {
        "estimatedTotalMinutes": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "risks": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "steps": {
          "items": {
            "$ref": "#/$defs/PlanStep"
          },
          "type": "array"
        }
      },
      "required": [
        "steps",
        "risks"
      ],
      "type": "object"
    },
    "PlanStep": {
      "properties": {
        "dependsOn": {
          "items": {
            "format": "uint32",
            "minimum": 0,
            "type": "integer"
          },
          "type": "array"
        },
        "description": {
          "type": [
            "string",
            "null"
          ]
        },
        "estimatedMinutes": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "title": {
          "type": "string"
        }
      },
      "required": [
        "title",
        "dependsOn"
      ],
      "type": "object"
    },
    "PublicApprovalEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "requested",
              "type": "string"
            },
            "request": {
              "$ref": "#/$defs/ApprovalRequest"
            }
          },
          "required": [
            "phase",
            "request"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "resolved",
              "type": "string"
            },
            "resolution": {
              "$ref": "#/$defs/PublicApprovalResolution"
            }
          },
          "required": [
            "phase",
            "resolution"
          ],
          "type": "object"
        }
      ]
    },
    "PublicApprovalResolution": {
      "additionalProperties": false,
      "properties": {
        "approvalId": {
          "type": "string"
        },
        "decision": {
          "$ref": "#/$defs/ApprovalDecision"
        },
        "reason": {
          "$ref": "#/$defs/ApprovalResolutionReason"
        },
        "runId": {
          "type": "string"
        },
        "toolCallId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "approvalId",
        "runId",
        "decision",
        "reason"
      ],
      "type": "object"
    },
    "PublicContextReceipt": {
      "additionalProperties": false,
      "properties": {
        "id": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/$defs/ReceiptKind"
        },
        "provenance": {
          "$ref": "#/$defs/ReceiptProvenance"
        },
        "state": {
          "$ref": "#/$defs/ReceiptState"
        },
        "summary": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "kind",
        "state",
        "provenance"
      ],
      "type": "object"
    },
    "PublicContextReceiptEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "created",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "promoted",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "quarantined",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        }
      ]
    },
    "PublicDaemonEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "session": {
              "$ref": "#/$defs/SessionEvent"
            }
          },
          "required": [
            "session"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "run": {
              "$ref": "#/$defs/RunEvent"
            }
          },
          "required": [
            "run"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "runReconciledOnStartup": {
              "$ref": "#/$defs/RunReconciledOnStartupEvent"
            }
          },
          "required": [
            "runReconciledOnStartup"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "approval": {
              "$ref": "#/$defs/PublicApprovalEvent"
            }
          },
          "required": [
            "approval"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "artifact": {
              "$ref": "#/$defs/ArtifactEvent"
            }
          },
          "required": [
            "artifact"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "contextReceipt": {
              "$ref": "#/$defs/PublicContextReceiptEvent"
            }
          },
          "required": [
            "contextReceipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "agentStream": {
              "$ref": "#/$defs/AgentStreamEvent"
            }
          },
          "required": [
            "agentStream"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "tokenUsageRecorded": {
              "$ref": "#/$defs/TokenUsageRecordedEvent"
            }
          },
          "required": [
            "tokenUsageRecorded"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "conflict": {
              "$ref": "#/$defs/ConflictEvent"
            }
          },
          "required": [
            "conflict"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "budget": {
              "$ref": "#/$defs/BudgetEvent"
            }
          },
          "required": [
            "budget"
          ],
          "type": "object"
        }
      ]
    },
    "PublicDaemonEventEnvelope": {
      "additionalProperties": false,
      "properties": {
        "daemonInstanceId": {
          "type": "string"
        },
        "event": {
          "$ref": "#/$defs/PublicDaemonEvent"
        },
        "occurredAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "sequence": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "sessionId": {
          "type": "string"
        }
      },
      "required": [
        "daemonInstanceId",
        "sessionId",
        "sequence",
        "occurredAtMs",
        "event"
      ],
      "type": "object"
    },
    "ReceiptKind": {
      "enum": [
        "evidence",
        "patch",
        "testOutput",
        "reviewFinding",
        "artifact",
        "risk",
        "blocker",
        "summary"
      ],
      "type": "string"
    },
    "ReceiptProvenance": {
      "description": "Provenance shape rules:\n- artifact-derived: only `artifact_id` is set; identity = (session, run, kind, artifact_id).\n- event-derived: both `event_seq` and `agent_turn_id` are set; identity = (session, run, kind, event_seq, agent_turn_id).\n- free-form: all identifying fields are None.\n\n`stream_cursor` is descriptive metadata (e.g. for UI navigation) and may be\npresent in any shape. It is never part of the unique identity.",
      "properties": {
        "agentTurnId": {
          "type": [
            "string",
            "null"
          ]
        },
        "artifactId": {
          "type": [
            "string",
            "null"
          ]
        },
        "eventSeq": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "streamCursor": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "type": "object"
    },
    "ReceiptState": {
      "enum": [
        "returned",
        "promoted",
        "quarantined"
      ],
      "type": "string"
    },
    "ReviewFinding": {
      "properties": {
        "file": {
          "type": [
            "string",
            "null"
          ]
        },
        "line": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "message": {
          "type": "string"
        },
        "severity": {
          "$ref": "#/$defs/FindingSeverity"
        },
        "suggestion": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "severity",
        "message"
      ],
      "type": "object"
    },
    "ReviewResult": {
      "properties": {
        "findings": {
          "items": {
            "$ref": "#/$defs/ReviewFinding"
          },
          "type": "array"
        },
        "risks": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "touchedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "verdict": {
          "$ref": "#/$defs/ReviewVerdict"
        }
      },
      "required": [
        "verdict",
        "findings",
        "risks",
        "touchedFiles"
      ],
      "type": "object"
    },
    "ReviewVerdict": {
      "enum": [
        "approve",
        "requestChanges",
        "needsHuman"
      ],
      "type": "string"
    },
    "RunEvent": {
      "properties": {
        "detail": {
          "type": "string"
        },
        "outputContract": {
          "anyOf": [
            {
              "$ref": "#/$defs/OutputContractKind"
            },
            {
              "type": "null"
            }
          ]
        },
        "recipeId": {
          "type": [
            "string",
            "null"
          ]
        },
        "result": {
          "anyOf": [
            {
              "$ref": "#/$defs/CapsuleResult"
            },
            {
              "type": "null"
            }
          ]
        },
        "runId": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/RunStatus"
        }
      },
      "required": [
        "runId",
        "status",
        "detail"
      ],
      "type": "object"
    },
    "RunFailureKind": {
      "enum": [
        "daemonRestartedWhileRunning"
      ],
      "type": "string"
    },
    "RunReconciledOnStartupEvent": {
      "properties": {
        "prevStatus": {
          "$ref": "#/$defs/RunStatus"
        },
        "reason": {
          "$ref": "#/$defs/RunFailureKind"
        },
        "runId": {
          "type": "string"
        }
      },
      "required": [
        "runId",
        "prevStatus",
        "reason"
      ],
      "type": "object"
    },
    "RunStatus": {
      "enum": [
        "queued",
        "running",
        "waitingForApproval",
        "completed",
        "failed",
        "budgetExceeded",
        "cancelled"
      ],
      "type": "string"
    },
    "RunSummary": {
      "properties": {
        "id": {
          "type": "string"
        },
        "objective": {
          "type": "string"
        },
        "runtimeProfileId": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/RunStatus"
        }
      },
      "required": [
        "id",
        "runtimeProfileId",
        "objective",
        "status"
      ],
      "type": "object"
    },
    "RuntimeLanePendingState": {
      "enum": [
        "queued",
        "waitingForApproval",
        "waitingForInput"
      ],
      "type": "string"
    },
    "SessionEvent": {
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/SessionStatus"
        }
      },
      "required": [
        "sessionId",
        "status"
      ],
      "type": "object"
    },
    "SessionOverview": {
      "properties": {
        "approvalAttention": {
          "$ref": "#/$defs/ApprovalAttentionState",
          "description": "Approval attention state owned by the daemon read model."
        },
        "isActive": {
          "description": "True when the session currently owns active or waiting work.",
          "type": "boolean"
        },
        "laneStatus": {
          "$ref": "#/$defs/SessionOverviewLaneStatus",
          "description": "Daemon-owned lane projection for operator-facing session/run state."
        },
        "lastActivityAtMs": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ],
          "description": "Timestamp of the newest daemon-owned activity item for this session."
        },
        "lastEventPreview": {
          "description": "Compact daemon-owned preview of the newest activity item for this session.",
          "type": [
            "string",
            "null"
          ]
        },
        "latestRun": {
          "anyOf": [
            {
              "$ref": "#/$defs/RunSummary"
            },
            {
              "type": "null"
            }
          ],
          "description": "Most recent run summary for this session, if one exists."
        },
        "pendingApprovalCount": {
          "description": "Count of approvals currently awaiting a decision for this session.",
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "recentActivity": {
          "description": "Recent public daemon activity for this session, ordered newest first.",
          "items": {
            "$ref": "#/$defs/PublicDaemonEventEnvelope"
          },
          "type": "array"
        },
        "session": {
          "$ref": "#/$defs/SessionSummary"
        }
      },
      "required": [
        "session",
        "laneStatus",
        "isActive",
        "approvalAttention",
        "pendingApprovalCount"
      ],
      "type": "object"
    },
    "SessionOverviewLaneStatus": {
      "enum": [
        "idle",
        "active",
        "waitingForApproval",
        "failed",
        "completed",
        "cancelled"
      ],
      "type": "string"
    },
    "SessionStatus": {
      "enum": [
        "idle",
        "running",
        "paused",
        "failed",
        "completed"
      ],
      "type": "string"
    },
    "SessionSummary": {
      "properties": {
        "id": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/SessionStatus"
        },
        "title": {
          "type": "string"
        }
      },
      "required": [
        "id",
        "title",
        "status"
      ],
      "type": "object"
    },
    "TestResult": {
      "properties": {
        "failed": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "failedTestNames": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "logReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "passed": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "skipped": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "total": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        }
      },
      "required": [
        "total",
        "passed",
        "failed",
        "skipped",
        "failedTestNames",
        "logReceiptIds"
      ],
      "type": "object"
    },
    "TokenUsageRecordedEvent": {
      "properties": {
        "cachedTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "capsuleId": {
          "type": [
            "string",
            "null"
          ]
        },
        "completionTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "model": {
          "type": "string"
        },
        "promptTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "provider": {
          "type": "string"
        },
        "reasoningTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "recordedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "runId": {
          "type": "string"
        }
      },
      "required": [
        "runId",
        "promptTokens",
        "completionTokens",
        "model",
        "provider",
        "recordedAtMs"
      ],
      "type": "object"
    },
    "WorkspaceMode": {
      "enum": [
        "readonly",
        "workspaceWrite",
        "worktreeWrite",
        "repoWriteWithApproval",
        "remoteWorker",
        "containerized",
        "ephemeral"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "sessions": {
      "items": {
        "$ref": "#/$defs/SessionOverview"
      },
      "type": "array"
    }
  },
  "title": "SessionOverviewResult",
  "type": "object"
},
  SessionOverviewLaneStatus: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "idle",
    "active",
    "waitingForApproval",
    "failed",
    "completed",
    "cancelled"
  ],
  "title": "SessionOverviewLaneStatus",
  "type": "string"
},
  ApprovalAttentionState: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "idle",
    "pending"
  ],
  "title": "ApprovalAttentionState",
  "type": "string"
},
  DaemonInitializeParams: {
  "$defs": {
    "DaemonClientCapabilities": {
      "properties": {
        "eventSubscriptions": {
          "type": "boolean"
        },
        "notifications": {
          "type": "boolean"
        }
      },
      "required": [
        "notifications",
        "eventSubscriptions"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "capabilities": {
      "$ref": "#/$defs/DaemonClientCapabilities"
    },
    "clientCredential": {
      "type": [
        "string",
        "null"
      ]
    },
    "clientName": {
      "type": "string"
    },
    "clientVersion": {
      "type": "string"
    },
    "protocolVersion": {
      "type": "string"
    }
  },
  "required": [
    "clientName",
    "clientVersion",
    "protocolVersion",
    "capabilities"
  ],
  "title": "DaemonInitializeParams",
  "type": "object"
},
  DaemonInitializeResult: {
  "$defs": {
    "DaemonServerCapabilities": {
      "properties": {
        "eventSubscriptions": {
          "type": "boolean"
        },
        "notifications": {
          "type": "boolean"
        }
      },
      "required": [
        "notifications",
        "eventSubscriptions"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "capabilities": {
      "$ref": "#/$defs/DaemonServerCapabilities"
    },
    "clientCredential": {
      "type": "string"
    },
    "daemonInstanceId": {
      "type": "string"
    },
    "daemonVersion": {
      "type": "string"
    },
    "protocolVersion": {
      "type": "string"
    }
  },
  "required": [
    "daemonInstanceId",
    "daemonVersion",
    "clientCredential",
    "protocolVersion",
    "capabilities"
  ],
  "title": "DaemonInitializeResult",
  "type": "object"
},
  DaemonRuntimeMode: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "local",
    "background"
  ],
  "title": "DaemonRuntimeMode",
  "type": "string"
},
  DaemonSessionOpenParams: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "title": {
      "type": "string"
    }
  },
  "required": [
    "title"
  ],
  "title": "DaemonSessionOpenParams",
  "type": "object"
},
  DaemonSessionOpenResult: {
  "$defs": {
    "DaemonEventCursor": {
      "description": "Resume cursor for `daemon.subscribe` and the `latestCursor` returned from\n`daemon.session.open` / `daemon.session.attach`.\n\nThis cursor is daemon-epoch-aware and scoped to one attached session.",
      "properties": {
        "daemonInstanceId": {
          "type": "string"
        },
        "sequence": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "sessionId": {
          "type": "string"
        }
      },
      "required": [
        "daemonInstanceId",
        "sessionId",
        "sequence"
      ],
      "type": "object"
    },
    "SessionStatus": {
      "enum": [
        "idle",
        "running",
        "paused",
        "failed",
        "completed"
      ],
      "type": "string"
    },
    "SessionSummary": {
      "properties": {
        "id": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/SessionStatus"
        },
        "title": {
          "type": "string"
        }
      },
      "required": [
        "id",
        "title",
        "status"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "latestCursor": {
      "anyOf": [
        {
          "$ref": "#/$defs/DaemonEventCursor"
        },
        {
          "type": "null"
        }
      ]
    },
    "session": {
      "$ref": "#/$defs/SessionSummary"
    },
    "sessionAuthority": {
      "type": "string"
    }
  },
  "required": [
    "session",
    "sessionAuthority"
  ],
  "title": "DaemonSessionOpenResult",
  "type": "object"
},
  DaemonSessionAttachParams: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "sessionAuthority": {
      "type": "string"
    },
    "sessionId": {
      "type": "string"
    }
  },
  "required": [
    "sessionId",
    "sessionAuthority"
  ],
  "title": "DaemonSessionAttachParams",
  "type": "object"
},
  DaemonSessionAttachResult: {
  "$defs": {
    "DaemonEventCursor": {
      "description": "Resume cursor for `daemon.subscribe` and the `latestCursor` returned from\n`daemon.session.open` / `daemon.session.attach`.\n\nThis cursor is daemon-epoch-aware and scoped to one attached session.",
      "properties": {
        "daemonInstanceId": {
          "type": "string"
        },
        "sequence": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "sessionId": {
          "type": "string"
        }
      },
      "required": [
        "daemonInstanceId",
        "sessionId",
        "sequence"
      ],
      "type": "object"
    },
    "SessionStatus": {
      "enum": [
        "idle",
        "running",
        "paused",
        "failed",
        "completed"
      ],
      "type": "string"
    },
    "SessionSummary": {
      "properties": {
        "id": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/SessionStatus"
        },
        "title": {
          "type": "string"
        }
      },
      "required": [
        "id",
        "title",
        "status"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "latestCursor": {
      "anyOf": [
        {
          "$ref": "#/$defs/DaemonEventCursor"
        },
        {
          "type": "null"
        }
      ]
    },
    "session": {
      "$ref": "#/$defs/SessionSummary"
    },
    "sessionAuthority": {
      "type": "string"
    }
  },
  "required": [
    "session",
    "sessionAuthority"
  ],
  "title": "DaemonSessionAttachResult",
  "type": "object"
},
  DaemonApprovalDecideParams: {
  "$defs": {
    "ApprovalDecision": {
      "enum": [
        "approved",
        "rejected"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "approvalId": {
      "type": "string"
    },
    "commentary": {
      "type": [
        "string",
        "null"
      ]
    },
    "decision": {
      "$ref": "#/$defs/ApprovalDecision"
    }
  },
  "required": [
    "approvalId",
    "decision"
  ],
  "title": "DaemonApprovalDecideParams",
  "type": "object"
},
  DaemonApprovalDecideResult: {
  "$defs": {
    "RunStatus": {
      "enum": [
        "queued",
        "running",
        "waitingForApproval",
        "completed",
        "failed",
        "budgetExceeded",
        "cancelled"
      ],
      "type": "string"
    },
    "RunSummary": {
      "properties": {
        "id": {
          "type": "string"
        },
        "objective": {
          "type": "string"
        },
        "runtimeProfileId": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/RunStatus"
        }
      },
      "required": [
        "id",
        "runtimeProfileId",
        "objective",
        "status"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "run": {
      "$ref": "#/$defs/RunSummary"
    }
  },
  "required": [
    "run"
  ],
  "title": "DaemonApprovalDecideResult",
  "type": "object"
},
  DaemonServerCapabilities: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "eventSubscriptions": {
      "type": "boolean"
    },
    "notifications": {
      "type": "boolean"
    }
  },
  "required": [
    "notifications",
    "eventSubscriptions"
  ],
  "title": "DaemonServerCapabilities",
  "type": "object"
},
  DaemonControlStatusResult: {
  "$defs": {
    "DaemonActualRuntimeMode": {
      "enum": [
        "stopped",
        "local",
        "background",
        "foreign"
      ],
      "type": "string"
    },
    "DaemonControlAction": {
      "enum": [
        "start",
        "stop",
        "enableBackground",
        "disableBackground",
        "reconcile"
      ],
      "type": "string"
    },
    "DaemonControlErrorCode": {
      "enum": [
        "unsupportedPlatform",
        "externalRuntime",
        "ownershipMismatch",
        "reconcileRequired",
        "transitionFailed"
      ],
      "type": "string"
    },
    "DaemonPendingTransitionKind": {
      "enum": [
        "enableBackground",
        "disableBackground",
        "recoverToLocal"
      ],
      "type": "string"
    },
    "DaemonPendingTransitionView": {
      "properties": {
        "kind": {
          "$ref": "#/$defs/DaemonPendingTransitionKind"
        },
        "opId": {
          "type": "string"
        }
      },
      "required": [
        "kind",
        "opId"
      ],
      "type": "object"
    },
    "DaemonRuntimeMode": {
      "enum": [
        "local",
        "background"
      ],
      "type": "string"
    },
    "DaemonTransitionStatus": {
      "enum": [
        "idle",
        "applying",
        "degradedReconcileRequired",
        "failedNoStateChange"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "actualMode": {
      "$ref": "#/$defs/DaemonActualRuntimeMode"
    },
    "allowedActions": {
      "items": {
        "$ref": "#/$defs/DaemonControlAction"
      },
      "type": "array"
    },
    "backgroundOptIn": {
      "type": "boolean"
    },
    "daemonVersion": {
      "description": "Running daemon version observed via daemon status.\n`None` means the daemon version is currently not observable, so UIs should omit it.",
      "type": [
        "string",
        "null"
      ]
    },
    "desiredMode": {
      "$ref": "#/$defs/DaemonRuntimeMode"
    },
    "errorCode": {
      "anyOf": [
        {
          "$ref": "#/$defs/DaemonControlErrorCode"
        },
        {
          "type": "null"
        }
      ]
    },
    "logPath": {
      "description": "Canonical daemon log path for this host/runtime configuration.",
      "type": "string"
    },
    "message": {
      "type": "string"
    },
    "pendingTransition": {
      "anyOf": [
        {
          "$ref": "#/$defs/DaemonPendingTransitionView"
        },
        {
          "type": "null"
        }
      ]
    },
    "protocolVersion": {
      "type": "string"
    },
    "reconcileRequired": {
      "type": "boolean"
    },
    "socketPath": {
      "type": "string"
    },
    "transitionStatus": {
      "$ref": "#/$defs/DaemonTransitionStatus"
    }
  },
  "required": [
    "backgroundOptIn",
    "desiredMode",
    "actualMode",
    "transitionStatus",
    "reconcileRequired",
    "allowedActions",
    "message",
    "socketPath",
    "logPath",
    "protocolVersion"
  ],
  "title": "DaemonControlStatusResult",
  "type": "object"
},
  DaemonStatusParams: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "DaemonStatusParams",
  "type": "object"
},
  DaemonStatusResult: {
  "$defs": {
    "DaemonRuntimeMode": {
      "enum": [
        "local",
        "background"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "daemonInstanceId": {
      "type": "string"
    },
    "logPath": {
      "type": "string"
    },
    "ready": {
      "type": "boolean"
    },
    "runtimeMode": {
      "$ref": "#/$defs/DaemonRuntimeMode"
    },
    "socketPath": {
      "type": "string"
    },
    "version": {
      "type": "string"
    }
  },
  "required": [
    "ready",
    "daemonInstanceId",
    "runtimeMode",
    "socketPath",
    "logPath",
    "version"
  ],
  "title": "DaemonStatusResult",
  "type": "object"
},
  DaemonDiagnosticsParams: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "DaemonDiagnosticsParams",
  "type": "object"
},
  DaemonDiagnostics: {
  "$defs": {
    "AgentRuntimeStrategyHealthStatus": {
      "enum": [
        "unknown",
        "ready",
        "degraded",
        "unavailable"
      ],
      "type": "string"
    },
    "DaemonDiagnosticError": {
      "properties": {
        "message": {
          "type": "string"
        },
        "occurredAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "source": {
          "type": "string"
        }
      },
      "required": [
        "occurredAtMs",
        "source",
        "message"
      ],
      "type": "object"
    },
    "DaemonDiagnosticTokenUsage": {
      "properties": {
        "cachedTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "completionTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "modelContextWindow": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "promptTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "reasoningTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "totalTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        }
      },
      "type": "object"
    },
    "DaemonProviderHealthDiagnostic": {
      "properties": {
        "displayName": {
          "type": "string"
        },
        "message": {
          "type": [
            "string",
            "null"
          ]
        },
        "providerId": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/AgentRuntimeStrategyHealthStatus"
        }
      },
      "required": [
        "providerId",
        "displayName",
        "status"
      ],
      "type": "object"
    },
    "DaemonSandboxCapabilitySnapshot": {
      "properties": {
        "appcontainer": {
          "type": "boolean"
        },
        "filesystemAllowlist": {
          "type": "boolean"
        },
        "helperAvailable": {
          "type": "boolean"
        },
        "networkDefaultDeny": {
          "type": "boolean"
        },
        "networkDestinationAllowlist": {
          "type": "boolean"
        },
        "os": {
          "type": "string"
        },
        "restrictedTokenJob": {
          "type": "boolean"
        },
        "sandboxKind": {
          "type": "string"
        }
      },
      "required": [
        "os",
        "sandboxKind",
        "helperAvailable",
        "restrictedTokenJob",
        "appcontainer",
        "filesystemAllowlist",
        "networkDefaultDeny",
        "networkDestinationAllowlist"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "claimCount": {
      "format": "uint32",
      "minimum": 0,
      "type": "integer"
    },
    "inFlightCapsuleRunCount": {
      "format": "uint32",
      "minimum": 0,
      "type": "integer"
    },
    "inFlightRpcCount": {
      "format": "uint32",
      "minimum": 0,
      "type": "integer"
    },
    "providerHealth": {
      "items": {
        "$ref": "#/$defs/DaemonProviderHealthDiagnostic"
      },
      "type": "array"
    },
    "recentErrorCount": {
      "format": "uint32",
      "minimum": 0,
      "type": "integer"
    },
    "recentErrors": {
      "items": {
        "$ref": "#/$defs/DaemonDiagnosticError"
      },
      "type": "array"
    },
    "sandbox": {
      "$ref": "#/$defs/DaemonSandboxCapabilitySnapshot"
    },
    "tokenUsage": {
      "$ref": "#/$defs/DaemonDiagnosticTokenUsage"
    },
    "uptimeMs": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    },
    "worktreeCount": {
      "format": "uint32",
      "minimum": 0,
      "type": "integer"
    }
  },
  "required": [
    "uptimeMs",
    "inFlightRpcCount",
    "inFlightCapsuleRunCount",
    "recentErrorCount",
    "recentErrors",
    "tokenUsage",
    "worktreeCount",
    "claimCount",
    "sandbox",
    "providerHealth"
  ],
  "title": "DaemonDiagnostics",
  "type": "object"
},
  DaemonDiagnosticError: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "message": {
      "type": "string"
    },
    "occurredAtMs": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    },
    "source": {
      "type": "string"
    }
  },
  "required": [
    "occurredAtMs",
    "source",
    "message"
  ],
  "title": "DaemonDiagnosticError",
  "type": "object"
},
  DaemonDiagnosticTokenUsage: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "cachedTokens": {
      "anyOf": [
        {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        {
          "type": "null"
        }
      ]
    },
    "completionTokens": {
      "anyOf": [
        {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        {
          "type": "null"
        }
      ]
    },
    "modelContextWindow": {
      "anyOf": [
        {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        {
          "type": "null"
        }
      ]
    },
    "promptTokens": {
      "anyOf": [
        {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        {
          "type": "null"
        }
      ]
    },
    "reasoningTokens": {
      "anyOf": [
        {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        {
          "type": "null"
        }
      ]
    },
    "totalTokens": {
      "anyOf": [
        {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        {
          "type": "null"
        }
      ]
    }
  },
  "title": "DaemonDiagnosticTokenUsage",
  "type": "object"
},
  DaemonSandboxCapabilitySnapshot: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "appcontainer": {
      "type": "boolean"
    },
    "filesystemAllowlist": {
      "type": "boolean"
    },
    "helperAvailable": {
      "type": "boolean"
    },
    "networkDefaultDeny": {
      "type": "boolean"
    },
    "networkDestinationAllowlist": {
      "type": "boolean"
    },
    "os": {
      "type": "string"
    },
    "restrictedTokenJob": {
      "type": "boolean"
    },
    "sandboxKind": {
      "type": "string"
    }
  },
  "required": [
    "os",
    "sandboxKind",
    "helperAvailable",
    "restrictedTokenJob",
    "appcontainer",
    "filesystemAllowlist",
    "networkDefaultDeny",
    "networkDestinationAllowlist"
  ],
  "title": "DaemonSandboxCapabilitySnapshot",
  "type": "object"
},
  DaemonProviderHealthDiagnostic: {
  "$defs": {
    "AgentRuntimeStrategyHealthStatus": {
      "enum": [
        "unknown",
        "ready",
        "degraded",
        "unavailable"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "displayName": {
      "type": "string"
    },
    "message": {
      "type": [
        "string",
        "null"
      ]
    },
    "providerId": {
      "type": "string"
    },
    "status": {
      "$ref": "#/$defs/AgentRuntimeStrategyHealthStatus"
    }
  },
  "required": [
    "providerId",
    "displayName",
    "status"
  ],
  "title": "DaemonProviderHealthDiagnostic",
  "type": "object"
},
  DaemonSubscribeParams: {
  "$defs": {
    "DaemonEventCursor": {
      "description": "Resume cursor for `daemon.subscribe` and the `latestCursor` returned from\n`daemon.session.open` / `daemon.session.attach`.\n\nThis cursor is daemon-epoch-aware and scoped to one attached session.",
      "properties": {
        "daemonInstanceId": {
          "type": "string"
        },
        "sequence": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "sessionId": {
          "type": "string"
        }
      },
      "required": [
        "daemonInstanceId",
        "sessionId",
        "sequence"
      ],
      "type": "object"
    },
    "DaemonEventKind": {
      "enum": [
        "session",
        "run",
        "runReconciledOnStartup",
        "approval",
        "artifact",
        "contextReceipt",
        "agentStream",
        "tokenUsageRecorded",
        "conflict",
        "budget"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "afterCursor": {
      "anyOf": [
        {
          "$ref": "#/$defs/DaemonEventCursor"
        },
        {
          "type": "null"
        }
      ]
    },
    "kinds": {
      "items": {
        "$ref": "#/$defs/DaemonEventKind"
      },
      "type": "array"
    }
  },
  "title": "DaemonSubscribeParams",
  "type": "object"
},
  DaemonSubscribeResult: {
  "$defs": {
    "DaemonEventCursor": {
      "description": "Resume cursor for `daemon.subscribe` and the `latestCursor` returned from\n`daemon.session.open` / `daemon.session.attach`.\n\nThis cursor is daemon-epoch-aware and scoped to one attached session.",
      "properties": {
        "daemonInstanceId": {
          "type": "string"
        },
        "sequence": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "sessionId": {
          "type": "string"
        }
      },
      "required": [
        "daemonInstanceId",
        "sessionId",
        "sequence"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "oneOf": [
    {
      "properties": {
        "latestCursor": {
          "anyOf": [
            {
              "$ref": "#/$defs/DaemonEventCursor"
            },
            {
              "type": "null"
            }
          ]
        },
        "status": {
          "const": "ready",
          "type": "string"
        }
      },
      "required": [
        "status"
      ],
      "type": "object"
    },
    {
      "properties": {
        "latestCursor": {
          "anyOf": [
            {
              "$ref": "#/$defs/DaemonEventCursor"
            },
            {
              "type": "null"
            }
          ]
        },
        "status": {
          "const": "historyGap",
          "type": "string"
        }
      },
      "required": [
        "status"
      ],
      "type": "object"
    }
  ],
  "title": "DaemonSubscribeResult"
},
  DelegateRequest: {
  "$defs": {
    "OutputContractKind": {
      "enum": [
        "debug",
        "patch",
        "review",
        "test",
        "plan",
        "custom"
      ],
      "type": "string"
    },
    "WorkspaceMode": {
      "enum": [
        "readonly",
        "workspaceWrite",
        "worktreeWrite",
        "repoWriteWithApproval",
        "remoteWorker",
        "containerized",
        "ephemeral"
      ],
      "type": "string"
    },
    "WorktreeCleanupPolicy": {
      "enum": [
        "deleteOnSuccess",
        "deleteOnTerminal",
        "keep",
        "manual"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "additionalProperties": false,
  "properties": {
    "cleanupPolicy": {
      "$ref": "#/$defs/WorktreeCleanupPolicy",
      "default": "deleteOnSuccess"
    },
    "modelId": {
      "type": [
        "string",
        "null"
      ]
    },
    "objective": {
      "type": "string"
    },
    "outputContract": {
      "anyOf": [
        {
          "$ref": "#/$defs/OutputContractKind"
        },
        {
          "type": "null"
        }
      ]
    },
    "plannedWriteFiles": {
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "recipeId": {
      "type": [
        "string",
        "null"
      ]
    },
    "sandboxProfile": {
      "type": [
        "string",
        "null"
      ]
    },
    "workspaceScope": {
      "$ref": "#/$defs/WorkspaceMode",
      "default": "worktreeWrite"
    }
  },
  "required": [
    "objective"
  ],
  "title": "DelegateRequest",
  "type": "object"
},
  ListApprovalsQuery: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "approvalId": {
      "type": [
        "string",
        "null"
      ]
    },
    "runId": {
      "type": [
        "string",
        "null"
      ]
    }
  },
  "title": "ListApprovalsQuery",
  "type": "object"
},
  ApprovalSnapshotResult: {
  "$defs": {
    "ApprovalRequest": {
      "properties": {
        "expiresAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "id": {
          "type": "string"
        },
        "reason": {
          "type": "string"
        },
        "requestedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "runId": {
          "type": "string"
        },
        "scope": {
          "$ref": "#/$defs/ApprovalScope"
        },
        "target": {
          "$ref": "#/$defs/ApprovalTarget"
        },
        "toolCallId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "runId",
        "scope",
        "requestedAtMs",
        "expiresAtMs",
        "target",
        "reason"
      ],
      "type": "object"
    },
    "ApprovalScope": {
      "enum": [
        "fileWrite",
        "processExec",
        "networkAccess"
      ],
      "type": "string"
    },
    "ApprovalTarget": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "toolCall",
              "type": "string"
            },
            "toolName": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "toolName"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "fileWrite",
              "type": "string"
            },
            "paths": {
              "items": {
                "type": "string"
              },
              "type": "array"
            }
          },
          "required": [
            "kind",
            "paths"
          ],
          "type": "object"
        },
        {
          "properties": {
            "command": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "processExec",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "host": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "networkAccess",
              "type": "string"
            },
            "protocol": {
              "type": [
                "string",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "childRunId": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "capsuleDispatch",
              "type": "string"
            },
            "workspaceScope": {
              "anyOf": [
                {
                  "$ref": "#/$defs/WorkspaceMode"
                },
                {
                  "type": "null"
                }
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "DaemonEventCursor": {
      "description": "Resume cursor for `daemon.subscribe` and the `latestCursor` returned from\n`daemon.session.open` / `daemon.session.attach`.\n\nThis cursor is daemon-epoch-aware and scoped to one attached session.",
      "properties": {
        "daemonInstanceId": {
          "type": "string"
        },
        "sequence": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "sessionId": {
          "type": "string"
        }
      },
      "required": [
        "daemonInstanceId",
        "sessionId",
        "sequence"
      ],
      "type": "object"
    },
    "WorkspaceMode": {
      "enum": [
        "readonly",
        "workspaceWrite",
        "worktreeWrite",
        "repoWriteWithApproval",
        "remoteWorker",
        "containerized",
        "ephemeral"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "items": {
      "items": {
        "$ref": "#/$defs/ApprovalRequest"
      },
      "type": "array"
    },
    "latestCursor": {
      "anyOf": [
        {
          "$ref": "#/$defs/DaemonEventCursor"
        },
        {
          "type": "null"
        }
      ]
    }
  },
  "required": [
    "items"
  ],
  "title": "ApprovalSnapshotResult",
  "type": "object"
},
  WorkItemKey: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "string",
  "type": "string"
},
  WorkItem: {
  "$defs": {
    "WorkItemStatus": {
      "enum": [
        "available",
        "dismissed",
        "triggered",
        "stale"
      ],
      "type": "string"
    },
    "WorkSource": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "gitHub",
              "type": "string"
            },
            "repo_name": {
              "type": "string"
            },
            "repo_owner": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "repo_owner",
            "repo_name"
          ],
          "type": "object"
        }
      ]
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "body": {
      "type": "string"
    },
    "externalId": {
      "type": "string"
    },
    "fetchedAtMs": {
      "format": "uint64",
      "minimum": 0,
      "type": "integer"
    },
    "key": {
      "type": "string"
    },
    "labels": {
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "source": {
      "$ref": "#/$defs/WorkSource"
    },
    "status": {
      "$ref": "#/$defs/WorkItemStatus"
    },
    "title": {
      "type": "string"
    },
    "triggeredRunId": {
      "type": [
        "string",
        "null"
      ]
    },
    "url": {
      "type": "string"
    }
  },
  "required": [
    "key",
    "source",
    "externalId",
    "title",
    "body",
    "labels",
    "url",
    "fetchedAtMs",
    "status"
  ],
  "title": "WorkItem",
  "type": "object"
},
  WorkItemListResult: {
  "$defs": {
    "WorkItem": {
      "properties": {
        "body": {
          "type": "string"
        },
        "externalId": {
          "type": "string"
        },
        "fetchedAtMs": {
          "format": "uint64",
          "minimum": 0,
          "type": "integer"
        },
        "key": {
          "type": "string"
        },
        "labels": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "source": {
          "$ref": "#/$defs/WorkSource"
        },
        "status": {
          "$ref": "#/$defs/WorkItemStatus"
        },
        "title": {
          "type": "string"
        },
        "triggeredRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "url": {
          "type": "string"
        }
      },
      "required": [
        "key",
        "source",
        "externalId",
        "title",
        "body",
        "labels",
        "url",
        "fetchedAtMs",
        "status"
      ],
      "type": "object"
    },
    "WorkItemStatus": {
      "enum": [
        "available",
        "dismissed",
        "triggered",
        "stale"
      ],
      "type": "string"
    },
    "WorkSource": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "gitHub",
              "type": "string"
            },
            "repo_name": {
              "type": "string"
            },
            "repo_owner": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "repo_owner",
            "repo_name"
          ],
          "type": "object"
        }
      ]
    },
    "WorkSourceSyncState": {
      "enum": [
        "disabled",
        "idle",
        "refreshQueued",
        "refreshing",
        "rateLimited",
        "error"
      ],
      "type": "string"
    },
    "WorkSourceSyncStatus": {
      "properties": {
        "detail": {
          "type": [
            "string",
            "null"
          ]
        },
        "lastFetchedAtMs": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "state": {
          "$ref": "#/$defs/WorkSourceSyncState"
        }
      },
      "required": [
        "state"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "items": {
      "items": {
        "$ref": "#/$defs/WorkItem"
      },
      "type": "array"
    },
    "sync": {
      "$ref": "#/$defs/WorkSourceSyncStatus"
    }
  },
  "required": [
    "items",
    "sync"
  ],
  "title": "WorkItemListResult",
  "type": "object"
},
  WorkItemRefreshParams: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "WorkItemRefreshParams",
  "type": "object"
},
  WorkItemDismissParams: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "key": {
      "type": "string"
    }
  },
  "required": [
    "key"
  ],
  "title": "WorkItemDismissParams",
  "type": "object"
},
  WorkItemDismissResult: {
  "$defs": {
    "WorkItem": {
      "properties": {
        "body": {
          "type": "string"
        },
        "externalId": {
          "type": "string"
        },
        "fetchedAtMs": {
          "format": "uint64",
          "minimum": 0,
          "type": "integer"
        },
        "key": {
          "type": "string"
        },
        "labels": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "source": {
          "$ref": "#/$defs/WorkSource"
        },
        "status": {
          "$ref": "#/$defs/WorkItemStatus"
        },
        "title": {
          "type": "string"
        },
        "triggeredRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "url": {
          "type": "string"
        }
      },
      "required": [
        "key",
        "source",
        "externalId",
        "title",
        "body",
        "labels",
        "url",
        "fetchedAtMs",
        "status"
      ],
      "type": "object"
    },
    "WorkItemStatus": {
      "enum": [
        "available",
        "dismissed",
        "triggered",
        "stale"
      ],
      "type": "string"
    },
    "WorkSource": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "gitHub",
              "type": "string"
            },
            "repo_name": {
              "type": "string"
            },
            "repo_owner": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "repo_owner",
            "repo_name"
          ],
          "type": "object"
        }
      ]
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "item": {
      "anyOf": [
        {
          "$ref": "#/$defs/WorkItem"
        },
        {
          "type": "null"
        }
      ]
    }
  },
  "title": "WorkItemDismissResult",
  "type": "object"
},
  WorkItemTriggerParams: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "key": {
      "type": "string"
    },
    "recipeId": {
      "type": [
        "string",
        "null"
      ]
    }
  },
  "required": [
    "key"
  ],
  "title": "WorkItemTriggerParams",
  "type": "object"
},
  WorkItemTriggerResult: {
  "$defs": {
    "RunStatus": {
      "enum": [
        "queued",
        "running",
        "waitingForApproval",
        "completed",
        "failed",
        "budgetExceeded",
        "cancelled"
      ],
      "type": "string"
    },
    "RunSummary": {
      "properties": {
        "id": {
          "type": "string"
        },
        "objective": {
          "type": "string"
        },
        "runtimeProfileId": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/RunStatus"
        }
      },
      "required": [
        "id",
        "runtimeProfileId",
        "objective",
        "status"
      ],
      "type": "object"
    },
    "WorkItem": {
      "properties": {
        "body": {
          "type": "string"
        },
        "externalId": {
          "type": "string"
        },
        "fetchedAtMs": {
          "format": "uint64",
          "minimum": 0,
          "type": "integer"
        },
        "key": {
          "type": "string"
        },
        "labels": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "source": {
          "$ref": "#/$defs/WorkSource"
        },
        "status": {
          "$ref": "#/$defs/WorkItemStatus"
        },
        "title": {
          "type": "string"
        },
        "triggeredRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "url": {
          "type": "string"
        }
      },
      "required": [
        "key",
        "source",
        "externalId",
        "title",
        "body",
        "labels",
        "url",
        "fetchedAtMs",
        "status"
      ],
      "type": "object"
    },
    "WorkItemStatus": {
      "enum": [
        "available",
        "dismissed",
        "triggered",
        "stale"
      ],
      "type": "string"
    },
    "WorkSource": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "gitHub",
              "type": "string"
            },
            "repo_name": {
              "type": "string"
            },
            "repo_owner": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "repo_owner",
            "repo_name"
          ],
          "type": "object"
        }
      ]
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "item": {
      "$ref": "#/$defs/WorkItem"
    },
    "run": {
      "$ref": "#/$defs/RunSummary"
    }
  },
  "required": [
    "item",
    "run"
  ],
  "title": "WorkItemTriggerResult",
  "type": "object"
},
  GetArtifactQuery: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "artifactId": {
      "type": "string"
    }
  },
  "required": [
    "artifactId"
  ],
  "title": "GetArtifactQuery",
  "type": "object"
},
  GetRunQuery: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "runId": {
      "type": "string"
    }
  },
  "required": [
    "runId"
  ],
  "title": "GetRunQuery",
  "type": "object"
},
  GetSessionQuery: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "GetSessionQuery",
  "type": "object"
},
  ListArtifactsQuery: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "artifactId": {
      "type": [
        "string",
        "null"
      ]
    },
    "runId": {
      "type": [
        "string",
        "null"
      ]
    }
  },
  "title": "ListArtifactsQuery",
  "type": "object"
},
  ArtifactSnapshotResult: {
  "$defs": {
    "ArtifactKind": {
      "enum": [
        "Transcript",
        "Patch",
        "FileSnapshot",
        "CommandLog"
      ],
      "type": "string"
    },
    "ArtifactSummary": {
      "properties": {
        "id": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/$defs/ArtifactKind"
        },
        "runId": {
          "type": "string"
        },
        "storagePath": {
          "type": "string"
        }
      },
      "required": [
        "id",
        "runId",
        "kind",
        "storagePath"
      ],
      "type": "object"
    },
    "DaemonEventCursor": {
      "description": "Resume cursor for `daemon.subscribe` and the `latestCursor` returned from\n`daemon.session.open` / `daemon.session.attach`.\n\nThis cursor is daemon-epoch-aware and scoped to one attached session.",
      "properties": {
        "daemonInstanceId": {
          "type": "string"
        },
        "sequence": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "sessionId": {
          "type": "string"
        }
      },
      "required": [
        "daemonInstanceId",
        "sessionId",
        "sequence"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "items": {
      "items": {
        "$ref": "#/$defs/ArtifactSummary"
      },
      "type": "array"
    },
    "latestCursor": {
      "anyOf": [
        {
          "$ref": "#/$defs/DaemonEventCursor"
        },
        {
          "type": "null"
        }
      ]
    }
  },
  "required": [
    "items"
  ],
  "title": "ArtifactSnapshotResult",
  "type": "object"
},
  ListReceiptsRequest: {
  "$defs": {
    "ReceiptKind": {
      "enum": [
        "evidence",
        "patch",
        "testOutput",
        "reviewFinding",
        "artifact",
        "risk",
        "blocker",
        "summary"
      ],
      "type": "string"
    },
    "ReceiptState": {
      "enum": [
        "returned",
        "promoted",
        "quarantined"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "kind": {
      "anyOf": [
        {
          "$ref": "#/$defs/ReceiptKind"
        },
        {
          "type": "null"
        }
      ]
    },
    "limit": {
      "format": "uint32",
      "minimum": 0,
      "type": [
        "integer",
        "null"
      ]
    },
    "parentRunId": {
      "type": [
        "string",
        "null"
      ]
    },
    "runId": {
      "type": [
        "string",
        "null"
      ]
    },
    "sessionId": {
      "type": "string"
    },
    "state": {
      "anyOf": [
        {
          "$ref": "#/$defs/ReceiptState"
        },
        {
          "type": "null"
        }
      ]
    }
  },
  "required": [
    "sessionId"
  ],
  "title": "ListReceiptsRequest",
  "type": "object"
},
  ListReceiptsResult: {
  "$defs": {
    "ContextReceipt": {
      "properties": {
        "createdAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "id": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/$defs/ReceiptKind"
        },
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "promotedAtMs": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "provenance": {
          "$ref": "#/$defs/ReceiptProvenance"
        },
        "quarantinedAtMs": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "runId": {
          "type": "string"
        },
        "sessionId": {
          "type": "string"
        },
        "state": {
          "$ref": "#/$defs/ReceiptState"
        },
        "summary": {
          "type": [
            "string",
            "null"
          ]
        },
        "title": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "sessionId",
        "runId",
        "kind",
        "provenance",
        "state",
        "createdAtMs"
      ],
      "type": "object"
    },
    "ReceiptKind": {
      "enum": [
        "evidence",
        "patch",
        "testOutput",
        "reviewFinding",
        "artifact",
        "risk",
        "blocker",
        "summary"
      ],
      "type": "string"
    },
    "ReceiptProvenance": {
      "description": "Provenance shape rules:\n- artifact-derived: only `artifact_id` is set; identity = (session, run, kind, artifact_id).\n- event-derived: both `event_seq` and `agent_turn_id` are set; identity = (session, run, kind, event_seq, agent_turn_id).\n- free-form: all identifying fields are None.\n\n`stream_cursor` is descriptive metadata (e.g. for UI navigation) and may be\npresent in any shape. It is never part of the unique identity.",
      "properties": {
        "agentTurnId": {
          "type": [
            "string",
            "null"
          ]
        },
        "artifactId": {
          "type": [
            "string",
            "null"
          ]
        },
        "eventSeq": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "streamCursor": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "type": "object"
    },
    "ReceiptState": {
      "enum": [
        "returned",
        "promoted",
        "quarantined"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "receipts": {
      "items": {
        "$ref": "#/$defs/ContextReceipt"
      },
      "type": "array"
    }
  },
  "required": [
    "receipts"
  ],
  "title": "ListReceiptsResult",
  "type": "object"
},
  PromoteReceiptRequest: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "receiptId": {
      "type": "string"
    },
    "sessionId": {
      "type": "string"
    }
  },
  "required": [
    "sessionId",
    "receiptId"
  ],
  "title": "PromoteReceiptRequest",
  "type": "object"
},
  QuarantineReceiptRequest: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "receiptId": {
      "type": "string"
    },
    "sessionId": {
      "type": "string"
    }
  },
  "required": [
    "sessionId",
    "receiptId"
  ],
  "title": "QuarantineReceiptRequest",
  "type": "object"
},
  OutputContractKind: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "debug",
    "patch",
    "review",
    "test",
    "plan",
    "custom"
  ],
  "title": "OutputContractKind",
  "type": "string"
},
  CapsuleResult: {
  "$defs": {
    "DebugResult": {
      "properties": {
        "blockers": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "confidence": {
          "maximum": 1,
          "minimum": 0,
          "type": "number"
        },
        "evidenceReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "patchReceiptId": {
          "type": [
            "string",
            "null"
          ]
        },
        "reproduced": {
          "type": "boolean"
        },
        "rootCause": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "reproduced",
        "evidenceReceiptIds",
        "confidence",
        "blockers"
      ],
      "type": "object"
    },
    "FindingSeverity": {
      "enum": [
        "low",
        "medium",
        "high",
        "critical"
      ],
      "type": "string"
    },
    "PatchResult": {
      "properties": {
        "blockers": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "passing": {
          "type": "boolean"
        },
        "patchReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "testsRunReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "touchedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        }
      },
      "required": [
        "patchReceiptIds",
        "touchedFiles",
        "testsRunReceiptIds",
        "passing",
        "blockers"
      ],
      "type": "object"
    },
    "PlanResult": {
      "properties": {
        "estimatedTotalMinutes": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "risks": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "steps": {
          "items": {
            "$ref": "#/$defs/PlanStep"
          },
          "type": "array"
        }
      },
      "required": [
        "steps",
        "risks"
      ],
      "type": "object"
    },
    "PlanStep": {
      "properties": {
        "dependsOn": {
          "items": {
            "format": "uint32",
            "minimum": 0,
            "type": "integer"
          },
          "type": "array"
        },
        "description": {
          "type": [
            "string",
            "null"
          ]
        },
        "estimatedMinutes": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "title": {
          "type": "string"
        }
      },
      "required": [
        "title",
        "dependsOn"
      ],
      "type": "object"
    },
    "ReviewFinding": {
      "properties": {
        "file": {
          "type": [
            "string",
            "null"
          ]
        },
        "line": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "message": {
          "type": "string"
        },
        "severity": {
          "$ref": "#/$defs/FindingSeverity"
        },
        "suggestion": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "severity",
        "message"
      ],
      "type": "object"
    },
    "ReviewResult": {
      "properties": {
        "findings": {
          "items": {
            "$ref": "#/$defs/ReviewFinding"
          },
          "type": "array"
        },
        "risks": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "touchedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "verdict": {
          "$ref": "#/$defs/ReviewVerdict"
        }
      },
      "required": [
        "verdict",
        "findings",
        "risks",
        "touchedFiles"
      ],
      "type": "object"
    },
    "ReviewVerdict": {
      "enum": [
        "approve",
        "requestChanges",
        "needsHuman"
      ],
      "type": "string"
    },
    "TestResult": {
      "properties": {
        "failed": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "failedTestNames": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "logReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "passed": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "skipped": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "total": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        }
      },
      "required": [
        "total",
        "passed",
        "failed",
        "skipped",
        "failedTestNames",
        "logReceiptIds"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "oneOf": [
    {
      "properties": {
        "kind": {
          "const": "debug",
          "type": "string"
        },
        "value": {
          "$ref": "#/$defs/DebugResult"
        }
      },
      "required": [
        "kind",
        "value"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "patch",
          "type": "string"
        },
        "value": {
          "$ref": "#/$defs/PatchResult"
        }
      },
      "required": [
        "kind",
        "value"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "review",
          "type": "string"
        },
        "value": {
          "$ref": "#/$defs/ReviewResult"
        }
      },
      "required": [
        "kind",
        "value"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "test",
          "type": "string"
        },
        "value": {
          "$ref": "#/$defs/TestResult"
        }
      },
      "required": [
        "kind",
        "value"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "plan",
          "type": "string"
        },
        "value": {
          "$ref": "#/$defs/PlanResult"
        }
      },
      "required": [
        "kind",
        "value"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "custom",
          "type": "string"
        },
        "value": true
      },
      "required": [
        "kind",
        "value"
      ],
      "type": "object"
    }
  ],
  "title": "CapsuleResult"
},
  DebugResult: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "blockers": {
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "confidence": {
      "maximum": 1,
      "minimum": 0,
      "type": "number"
    },
    "evidenceReceiptIds": {
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "patchReceiptId": {
      "type": [
        "string",
        "null"
      ]
    },
    "reproduced": {
      "type": "boolean"
    },
    "rootCause": {
      "type": [
        "string",
        "null"
      ]
    }
  },
  "required": [
    "reproduced",
    "evidenceReceiptIds",
    "confidence",
    "blockers"
  ],
  "title": "DebugResult",
  "type": "object"
},
  PatchResult: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "blockers": {
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "passing": {
      "type": "boolean"
    },
    "patchReceiptIds": {
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "testsRunReceiptIds": {
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "touchedFiles": {
      "items": {
        "type": "string"
      },
      "type": "array"
    }
  },
  "required": [
    "patchReceiptIds",
    "touchedFiles",
    "testsRunReceiptIds",
    "passing",
    "blockers"
  ],
  "title": "PatchResult",
  "type": "object"
},
  ReviewResult: {
  "$defs": {
    "FindingSeverity": {
      "enum": [
        "low",
        "medium",
        "high",
        "critical"
      ],
      "type": "string"
    },
    "ReviewFinding": {
      "properties": {
        "file": {
          "type": [
            "string",
            "null"
          ]
        },
        "line": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "message": {
          "type": "string"
        },
        "severity": {
          "$ref": "#/$defs/FindingSeverity"
        },
        "suggestion": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "severity",
        "message"
      ],
      "type": "object"
    },
    "ReviewVerdict": {
      "enum": [
        "approve",
        "requestChanges",
        "needsHuman"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "findings": {
      "items": {
        "$ref": "#/$defs/ReviewFinding"
      },
      "type": "array"
    },
    "risks": {
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "touchedFiles": {
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "verdict": {
      "$ref": "#/$defs/ReviewVerdict"
    }
  },
  "required": [
    "verdict",
    "findings",
    "risks",
    "touchedFiles"
  ],
  "title": "ReviewResult",
  "type": "object"
},
  ReviewVerdict: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "approve",
    "requestChanges",
    "needsHuman"
  ],
  "title": "ReviewVerdict",
  "type": "string"
},
  ReviewFinding: {
  "$defs": {
    "FindingSeverity": {
      "enum": [
        "low",
        "medium",
        "high",
        "critical"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "file": {
      "type": [
        "string",
        "null"
      ]
    },
    "line": {
      "format": "uint32",
      "minimum": 0,
      "type": [
        "integer",
        "null"
      ]
    },
    "message": {
      "type": "string"
    },
    "severity": {
      "$ref": "#/$defs/FindingSeverity"
    },
    "suggestion": {
      "type": [
        "string",
        "null"
      ]
    }
  },
  "required": [
    "severity",
    "message"
  ],
  "title": "ReviewFinding",
  "type": "object"
},
  FindingSeverity: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "low",
    "medium",
    "high",
    "critical"
  ],
  "title": "FindingSeverity",
  "type": "string"
},
  TestResult: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "failed": {
      "format": "uint32",
      "minimum": 0,
      "type": "integer"
    },
    "failedTestNames": {
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "logReceiptIds": {
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "passed": {
      "format": "uint32",
      "minimum": 0,
      "type": "integer"
    },
    "skipped": {
      "format": "uint32",
      "minimum": 0,
      "type": "integer"
    },
    "total": {
      "format": "uint32",
      "minimum": 0,
      "type": "integer"
    }
  },
  "required": [
    "total",
    "passed",
    "failed",
    "skipped",
    "failedTestNames",
    "logReceiptIds"
  ],
  "title": "TestResult",
  "type": "object"
},
  PlanResult: {
  "$defs": {
    "PlanStep": {
      "properties": {
        "dependsOn": {
          "items": {
            "format": "uint32",
            "minimum": 0,
            "type": "integer"
          },
          "type": "array"
        },
        "description": {
          "type": [
            "string",
            "null"
          ]
        },
        "estimatedMinutes": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "title": {
          "type": "string"
        }
      },
      "required": [
        "title",
        "dependsOn"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "estimatedTotalMinutes": {
      "format": "uint32",
      "minimum": 0,
      "type": [
        "integer",
        "null"
      ]
    },
    "risks": {
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "steps": {
      "items": {
        "$ref": "#/$defs/PlanStep"
      },
      "type": "array"
    }
  },
  "required": [
    "steps",
    "risks"
  ],
  "title": "PlanResult",
  "type": "object"
},
  PlanStep: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "dependsOn": {
      "items": {
        "format": "uint32",
        "minimum": 0,
        "type": "integer"
      },
      "type": "array"
    },
    "description": {
      "type": [
        "string",
        "null"
      ]
    },
    "estimatedMinutes": {
      "format": "uint32",
      "minimum": 0,
      "type": [
        "integer",
        "null"
      ]
    },
    "title": {
      "type": "string"
    }
  },
  "required": [
    "title",
    "dependsOn"
  ],
  "title": "PlanStep",
  "type": "object"
},
  ValidationError: {
  "$defs": {
    "OutputContractKind": {
      "enum": [
        "debug",
        "patch",
        "review",
        "test",
        "plan",
        "custom"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "oneOf": [
    {
      "properties": {
        "kind": {
          "const": "kindMismatch",
          "type": "string"
        },
        "value": {
          "properties": {
            "expected": {
              "$ref": "#/$defs/OutputContractKind"
            },
            "got": {
              "$ref": "#/$defs/OutputContractKind"
            }
          },
          "required": [
            "expected",
            "got"
          ],
          "type": "object"
        }
      },
      "required": [
        "kind",
        "value"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "confidenceOutOfRange",
          "type": "string"
        },
        "value": {
          "properties": {
            "value": {
              "type": "number"
            }
          },
          "required": [
            "value"
          ],
          "type": "object"
        }
      },
      "required": [
        "kind",
        "value"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "invalidReceiptId",
          "type": "string"
        },
        "value": {
          "type": "string"
        }
      },
      "required": [
        "kind",
        "value"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "testCountsInconsistent",
          "type": "string"
        },
        "value": {
          "properties": {
            "sumOfParts": {
              "format": "uint32",
              "minimum": 0,
              "type": "integer"
            },
            "total": {
              "format": "uint32",
              "minimum": 0,
              "type": "integer"
            }
          },
          "required": [
            "total",
            "sumOfParts"
          ],
          "type": "object"
        }
      },
      "required": [
        "kind",
        "value"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "testCountsOverflow",
          "type": "string"
        },
        "value": {
          "properties": {
            "failed": {
              "format": "uint32",
              "minimum": 0,
              "type": "integer"
            },
            "passed": {
              "format": "uint32",
              "minimum": 0,
              "type": "integer"
            },
            "skipped": {
              "format": "uint32",
              "minimum": 0,
              "type": "integer"
            }
          },
          "required": [
            "passed",
            "failed",
            "skipped"
          ],
          "type": "object"
        }
      },
      "required": [
        "kind",
        "value"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "planStepDependencyOutOfRange",
          "type": "string"
        },
        "value": {
          "properties": {
            "dependency": {
              "format": "uint32",
              "minimum": 0,
              "type": "integer"
            },
            "stepIndex": {
              "format": "uint32",
              "minimum": 0,
              "type": "integer"
            },
            "totalSteps": {
              "format": "uint32",
              "minimum": 0,
              "type": "integer"
            }
          },
          "required": [
            "stepIndex",
            "dependency",
            "totalSteps"
          ],
          "type": "object"
        }
      },
      "required": [
        "kind",
        "value"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "custom",
          "type": "string"
        },
        "value": {
          "type": "string"
        }
      },
      "required": [
        "kind",
        "value"
      ],
      "type": "object"
    }
  ],
  "title": "ValidationError"
},
  CapsuleRecipe: {
  "$defs": {
    "OutputContractKind": {
      "enum": [
        "debug",
        "patch",
        "review",
        "test",
        "plan",
        "custom"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "contract": {
      "$ref": "#/$defs/OutputContractKind"
    },
    "defaultModel": {
      "type": [
        "string",
        "null"
      ]
    },
    "defaultSandboxProfile": {
      "type": [
        "string",
        "null"
      ]
    },
    "description": {
      "type": [
        "string",
        "null"
      ]
    },
    "id": {
      "type": "string"
    },
    "name": {
      "type": "string"
    },
    "promptTemplate": {
      "type": "string"
    }
  },
  "required": [
    "id",
    "name",
    "contract",
    "promptTemplate"
  ],
  "title": "CapsuleRecipe",
  "type": "object"
},
  ListRecipesParams: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "ListRecipesParams",
  "type": "object"
},
  RecipeListResponse: {
  "$defs": {
    "CapsuleRecipe": {
      "properties": {
        "contract": {
          "$ref": "#/$defs/OutputContractKind"
        },
        "defaultModel": {
          "type": [
            "string",
            "null"
          ]
        },
        "defaultSandboxProfile": {
          "type": [
            "string",
            "null"
          ]
        },
        "description": {
          "type": [
            "string",
            "null"
          ]
        },
        "id": {
          "type": "string"
        },
        "name": {
          "type": "string"
        },
        "promptTemplate": {
          "type": "string"
        }
      },
      "required": [
        "id",
        "name",
        "contract",
        "promptTemplate"
      ],
      "type": "object"
    },
    "OutputContractKind": {
      "enum": [
        "debug",
        "patch",
        "review",
        "test",
        "plan",
        "custom"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "recipes": {
      "items": {
        "$ref": "#/$defs/CapsuleRecipe"
      },
      "type": "array"
    }
  },
  "required": [
    "recipes"
  ],
  "title": "RecipeListResponse",
  "type": "object"
},
  RecipeValidationError: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "oneOf": [
    {
      "properties": {
        "kind": {
          "const": "emptyId",
          "type": "string"
        }
      },
      "required": [
        "kind"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "emptyName",
          "type": "string"
        }
      },
      "required": [
        "kind"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "emptyTemplate",
          "type": "string"
        }
      },
      "required": [
        "kind"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "emptyDefaultModel",
          "type": "string"
        }
      },
      "required": [
        "kind"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "emptyDefaultSandboxProfile",
          "type": "string"
        }
      },
      "required": [
        "kind"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "invalidIdCharacters",
          "type": "string"
        },
        "value": {
          "properties": {
            "value": {
              "type": "string"
            }
          },
          "required": [
            "value"
          ],
          "type": "object"
        }
      },
      "required": [
        "kind",
        "value"
      ],
      "type": "object"
    }
  ],
  "title": "RecipeValidationError"
},
  RecipeResolutionError: {
  "$defs": {
    "OutputContractKind": {
      "enum": [
        "debug",
        "patch",
        "review",
        "test",
        "plan",
        "custom"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "oneOf": [
    {
      "properties": {
        "kind": {
          "const": "unknownRecipeId",
          "type": "string"
        },
        "recipeId": {
          "type": "string"
        }
      },
      "required": [
        "kind",
        "recipeId"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "recipeContractConflict",
          "type": "string"
        },
        "recipeContract": {
          "$ref": "#/$defs/OutputContractKind"
        },
        "recipeId": {
          "type": "string"
        },
        "requestContract": {
          "$ref": "#/$defs/OutputContractKind"
        }
      },
      "required": [
        "kind",
        "recipeId",
        "recipeContract",
        "requestContract"
      ],
      "type": "object"
    }
  ],
  "title": "RecipeResolutionError"
},
  ListRunsQuery: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "ListRunsQuery",
  "type": "object"
},
  RunListFilter: {
  "$defs": {
    "RunHarnessKind": {
      "enum": [
        "unknown",
        "native",
        "acp",
        "codexAppServer"
      ],
      "type": "string"
    },
    "RunStatus": {
      "enum": [
        "queued",
        "running",
        "waitingForApproval",
        "completed",
        "failed",
        "budgetExceeded",
        "cancelled"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "harness": {
      "items": {
        "$ref": "#/$defs/RunHarnessKind"
      },
      "type": [
        "array",
        "null"
      ]
    },
    "parentRunId": {
      "type": [
        "string",
        "null"
      ]
    },
    "status": {
      "items": {
        "$ref": "#/$defs/RunStatus"
      },
      "type": [
        "array",
        "null"
      ]
    }
  },
  "title": "RunListFilter",
  "type": "object"
},
  ListNativeRunsRequest: {
  "$defs": {
    "RunHarnessKind": {
      "enum": [
        "unknown",
        "native",
        "acp",
        "codexAppServer"
      ],
      "type": "string"
    },
    "RunListFilter": {
      "properties": {
        "harness": {
          "items": {
            "$ref": "#/$defs/RunHarnessKind"
          },
          "type": [
            "array",
            "null"
          ]
        },
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "status": {
          "items": {
            "$ref": "#/$defs/RunStatus"
          },
          "type": [
            "array",
            "null"
          ]
        }
      },
      "type": "object"
    },
    "RunStatus": {
      "enum": [
        "queued",
        "running",
        "waitingForApproval",
        "completed",
        "failed",
        "budgetExceeded",
        "cancelled"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "cursor": {
      "type": [
        "string",
        "null"
      ]
    },
    "filter": {
      "anyOf": [
        {
          "$ref": "#/$defs/RunListFilter"
        },
        {
          "type": "null"
        }
      ]
    },
    "limit": {
      "format": "uint32",
      "minimum": 0,
      "type": "integer"
    }
  },
  "required": [
    "limit"
  ],
  "title": "ListNativeRunsRequest",
  "type": "object"
},
  RunListEntry: {
  "$defs": {
    "ConflictSummary": {
      "properties": {
        "files": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "warningCount": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        }
      },
      "required": [
        "warningCount",
        "files"
      ],
      "type": "object"
    },
    "OutputContractKind": {
      "enum": [
        "debug",
        "patch",
        "review",
        "test",
        "plan",
        "custom"
      ],
      "type": "string"
    },
    "RunHarnessKind": {
      "enum": [
        "unknown",
        "native",
        "acp",
        "codexAppServer"
      ],
      "type": "string"
    },
    "RunStatus": {
      "enum": [
        "queued",
        "running",
        "waitingForApproval",
        "completed",
        "failed",
        "budgetExceeded",
        "cancelled"
      ],
      "type": "string"
    },
    "WorktreeCleanupPolicy": {
      "enum": [
        "deleteOnSuccess",
        "deleteOnTerminal",
        "keep",
        "manual"
      ],
      "type": "string"
    },
    "WorktreeInfo": {
      "properties": {
        "branch": {
          "type": "string"
        },
        "cleanupPolicy": {
          "$ref": "#/$defs/WorktreeCleanupPolicy"
        },
        "path": {
          "type": "string"
        }
      },
      "required": [
        "path",
        "branch",
        "cleanupPolicy"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "claimedFiles": {
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "conflictSummary": {
      "anyOf": [
        {
          "$ref": "#/$defs/ConflictSummary"
        },
        {
          "type": "null"
        }
      ]
    },
    "endedAtMs": {
      "anyOf": [
        {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        {
          "type": "null"
        }
      ]
    },
    "harness": {
      "$ref": "#/$defs/RunHarnessKind"
    },
    "id": {
      "type": "string"
    },
    "lastEventSeq": {
      "anyOf": [
        {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        {
          "type": "null"
        }
      ]
    },
    "objectivePreview": {
      "type": [
        "string",
        "null"
      ]
    },
    "outputContract": {
      "anyOf": [
        {
          "$ref": "#/$defs/OutputContractKind"
        },
        {
          "type": "null"
        }
      ]
    },
    "parentRunId": {
      "type": [
        "string",
        "null"
      ]
    },
    "recipeId": {
      "type": [
        "string",
        "null"
      ]
    },
    "startedAtMs": {
      "anyOf": [
        {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        {
          "type": "null"
        }
      ]
    },
    "status": {
      "$ref": "#/$defs/RunStatus"
    },
    "workspaceInfo": {
      "anyOf": [
        {
          "$ref": "#/$defs/WorktreeInfo"
        },
        {
          "type": "null"
        }
      ]
    }
  },
  "required": [
    "id",
    "harness",
    "status"
  ],
  "title": "RunListEntry",
  "type": "object"
},
  ListNativeRunsResult: {
  "$defs": {
    "ConflictSummary": {
      "properties": {
        "files": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "warningCount": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        }
      },
      "required": [
        "warningCount",
        "files"
      ],
      "type": "object"
    },
    "OutputContractKind": {
      "enum": [
        "debug",
        "patch",
        "review",
        "test",
        "plan",
        "custom"
      ],
      "type": "string"
    },
    "RunHarnessKind": {
      "enum": [
        "unknown",
        "native",
        "acp",
        "codexAppServer"
      ],
      "type": "string"
    },
    "RunListEntry": {
      "properties": {
        "claimedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "conflictSummary": {
          "anyOf": [
            {
              "$ref": "#/$defs/ConflictSummary"
            },
            {
              "type": "null"
            }
          ]
        },
        "endedAtMs": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "harness": {
          "$ref": "#/$defs/RunHarnessKind"
        },
        "id": {
          "type": "string"
        },
        "lastEventSeq": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "objectivePreview": {
          "type": [
            "string",
            "null"
          ]
        },
        "outputContract": {
          "anyOf": [
            {
              "$ref": "#/$defs/OutputContractKind"
            },
            {
              "type": "null"
            }
          ]
        },
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "recipeId": {
          "type": [
            "string",
            "null"
          ]
        },
        "startedAtMs": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "status": {
          "$ref": "#/$defs/RunStatus"
        },
        "workspaceInfo": {
          "anyOf": [
            {
              "$ref": "#/$defs/WorktreeInfo"
            },
            {
              "type": "null"
            }
          ]
        }
      },
      "required": [
        "id",
        "harness",
        "status"
      ],
      "type": "object"
    },
    "RunStatus": {
      "enum": [
        "queued",
        "running",
        "waitingForApproval",
        "completed",
        "failed",
        "budgetExceeded",
        "cancelled"
      ],
      "type": "string"
    },
    "WorktreeCleanupPolicy": {
      "enum": [
        "deleteOnSuccess",
        "deleteOnTerminal",
        "keep",
        "manual"
      ],
      "type": "string"
    },
    "WorktreeInfo": {
      "properties": {
        "branch": {
          "type": "string"
        },
        "cleanupPolicy": {
          "$ref": "#/$defs/WorktreeCleanupPolicy"
        },
        "path": {
          "type": "string"
        }
      },
      "required": [
        "path",
        "branch",
        "cleanupPolicy"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "nextCursor": {
      "type": [
        "string",
        "null"
      ]
    },
    "runs": {
      "items": {
        "$ref": "#/$defs/RunListEntry"
      },
      "type": "array"
    }
  },
  "required": [
    "runs"
  ],
  "title": "ListNativeRunsResult",
  "type": "object"
},
  GetRunTimelineQuery: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "afterSeq": {
      "anyOf": [
        {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        {
          "type": "null"
        }
      ]
    },
    "limit": {
      "format": "uint32",
      "minimum": 0,
      "type": [
        "integer",
        "null"
      ]
    },
    "rootRunId": {
      "type": "string"
    },
    "sessionId": {
      "type": "string"
    }
  },
  "required": [
    "sessionId",
    "rootRunId"
  ],
  "title": "GetRunTimelineQuery",
  "type": "object"
},
  RunTimeline: {
  "$defs": {
    "AgentToolCallOutcome": {
      "enum": [
        "completed",
        "failed",
        "cancelled"
      ],
      "type": "string"
    },
    "ApprovalDecision": {
      "enum": [
        "approved",
        "rejected"
      ],
      "type": "string"
    },
    "ApprovalScope": {
      "enum": [
        "fileWrite",
        "processExec",
        "networkAccess"
      ],
      "type": "string"
    },
    "ArtifactKind": {
      "enum": [
        "Transcript",
        "Patch",
        "FileSnapshot",
        "CommandLog"
      ],
      "type": "string"
    },
    "BudgetMetric": {
      "enum": [
        "tokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "string"
    },
    "BudgetScope": {
      "enum": [
        "run",
        "parentAggregate"
      ],
      "type": "string"
    },
    "ConflictSeverity": {
      "enum": [
        "informational",
        "warning"
      ],
      "type": "string"
    },
    "ConflictWarning": {
      "properties": {
        "conflicts": {
          "items": {
            "$ref": "#/$defs/FileClaimConflict"
          },
          "type": "array"
        },
        "requestingCapsule": {
          "type": "string"
        },
        "severity": {
          "$ref": "#/$defs/ConflictSeverity"
        }
      },
      "required": [
        "requestingCapsule",
        "severity",
        "conflicts"
      ],
      "type": "object"
    },
    "FileClaimConflict": {
      "properties": {
        "file": {
          "type": "string"
        },
        "holdingCapsule": {
          "type": "string"
        },
        "holdingKind": {
          "$ref": "#/$defs/FileClaimKind"
        }
      },
      "required": [
        "file",
        "holdingCapsule",
        "holdingKind"
      ],
      "type": "object"
    },
    "FileClaimKind": {
      "enum": [
        "write"
      ],
      "type": "string"
    },
    "OutputContractKind": {
      "enum": [
        "debug",
        "patch",
        "review",
        "test",
        "plan",
        "custom"
      ],
      "type": "string"
    },
    "ReceiptKind": {
      "enum": [
        "evidence",
        "patch",
        "testOutput",
        "reviewFinding",
        "artifact",
        "risk",
        "blocker",
        "summary"
      ],
      "type": "string"
    },
    "ReceiptState": {
      "enum": [
        "returned",
        "promoted",
        "quarantined"
      ],
      "type": "string"
    },
    "RunStatus": {
      "enum": [
        "queued",
        "running",
        "waitingForApproval",
        "completed",
        "failed",
        "budgetExceeded",
        "cancelled"
      ],
      "type": "string"
    },
    "RunTimelineEvent": {
      "properties": {
        "kind": {
          "$ref": "#/$defs/RunTimelineEventKind"
        },
        "label": {
          "type": "string"
        },
        "occurredAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "payload": {
          "$ref": "#/$defs/RunTimelineEventPayload"
        },
        "runId": {
          "type": "string"
        },
        "seq": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "status": {
          "anyOf": [
            {
              "$ref": "#/$defs/RunStatus"
            },
            {
              "type": "null"
            }
          ]
        }
      },
      "required": [
        "seq",
        "occurredAtMs",
        "runId",
        "kind",
        "label",
        "payload"
      ],
      "type": "object"
    },
    "RunTimelineEventKind": {
      "enum": [
        "runStatus",
        "approvalRequested",
        "approvalResolved",
        "claimConflict",
        "budgetExceeded",
        "tokenUsage",
        "toolCall",
        "artifact",
        "receipt",
        "agentStream"
      ],
      "type": "string"
    },
    "RunTimelineEventPayload": {
      "oneOf": [
        {
          "properties": {
            "detail": {
              "type": "string"
            },
            "kind": {
              "const": "run",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "detail"
          ],
          "type": "object"
        },
        {
          "properties": {
            "approvalId": {
              "type": "string"
            },
            "kind": {
              "const": "approvalRequested",
              "type": "string"
            },
            "scope": {
              "$ref": "#/$defs/ApprovalScope"
            }
          },
          "required": [
            "kind",
            "approvalId",
            "scope"
          ],
          "type": "object"
        },
        {
          "properties": {
            "approvalId": {
              "type": "string"
            },
            "decision": {
              "$ref": "#/$defs/ApprovalDecision"
            },
            "kind": {
              "const": "approvalResolved",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "approvalId",
            "decision"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "conflict",
              "type": "string"
            },
            "warning": {
              "$ref": "#/$defs/ConflictWarning"
            }
          },
          "required": [
            "kind",
            "warning"
          ],
          "type": "object"
        },
        {
          "properties": {
            "actual": {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            "kind": {
              "const": "budgetExceeded",
              "type": "string"
            },
            "limit": {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            "metric": {
              "$ref": "#/$defs/BudgetMetric"
            },
            "scope": {
              "$ref": "#/$defs/BudgetScope"
            }
          },
          "required": [
            "kind",
            "scope",
            "metric",
            "limit",
            "actual"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "tokenUsage",
              "type": "string"
            },
            "usage": {
              "$ref": "#/$defs/TokenUsageRecordedEvent"
            }
          },
          "required": [
            "kind",
            "usage"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "toolCall",
              "type": "string"
            },
            "outcome": {
              "anyOf": [
                {
                  "$ref": "#/$defs/AgentToolCallOutcome"
                },
                {
                  "type": "null"
                }
              ]
            },
            "toolName": {
              "type": [
                "string",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "artifactId": {
              "type": "string"
            },
            "artifactKind": {
              "$ref": "#/$defs/ArtifactKind"
            },
            "kind": {
              "const": "artifact",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "artifactId",
            "artifactKind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "receipt",
              "type": "string"
            },
            "receiptId": {
              "type": "string"
            },
            "receiptKind": {
              "$ref": "#/$defs/ReceiptKind"
            },
            "receiptState": {
              "$ref": "#/$defs/ReceiptState"
            }
          },
          "required": [
            "kind",
            "receiptId",
            "receiptKind",
            "receiptState"
          ],
          "type": "object"
        },
        {
          "properties": {
            "frameKind": {
              "type": "string"
            },
            "kind": {
              "const": "agentStream",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "frameKind"
          ],
          "type": "object"
        }
      ]
    },
    "RunTimelineRun": {
      "properties": {
        "claimedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "depth": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "endedAtMs": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "outputContract": {
          "anyOf": [
            {
              "$ref": "#/$defs/OutputContractKind"
            },
            {
              "type": "null"
            }
          ]
        },
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "recipeId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "startedAtMs": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "status": {
          "$ref": "#/$defs/RunStatus"
        },
        "workspaceInfo": {
          "anyOf": [
            {
              "$ref": "#/$defs/WorktreeInfo"
            },
            {
              "type": "null"
            }
          ]
        }
      },
      "required": [
        "runId",
        "depth",
        "status"
      ],
      "type": "object"
    },
    "TokenUsageRecordedEvent": {
      "properties": {
        "cachedTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "capsuleId": {
          "type": [
            "string",
            "null"
          ]
        },
        "completionTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "model": {
          "type": "string"
        },
        "promptTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "provider": {
          "type": "string"
        },
        "reasoningTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "recordedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "runId": {
          "type": "string"
        }
      },
      "required": [
        "runId",
        "promptTokens",
        "completionTokens",
        "model",
        "provider",
        "recordedAtMs"
      ],
      "type": "object"
    },
    "WorktreeCleanupPolicy": {
      "enum": [
        "deleteOnSuccess",
        "deleteOnTerminal",
        "keep",
        "manual"
      ],
      "type": "string"
    },
    "WorktreeInfo": {
      "properties": {
        "branch": {
          "type": "string"
        },
        "cleanupPolicy": {
          "$ref": "#/$defs/WorktreeCleanupPolicy"
        },
        "path": {
          "type": "string"
        }
      },
      "required": [
        "path",
        "branch",
        "cleanupPolicy"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "events": {
      "items": {
        "$ref": "#/$defs/RunTimelineEvent"
      },
      "type": "array"
    },
    "latestEventSeq": {
      "anyOf": [
        {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        {
          "type": "null"
        }
      ]
    },
    "rootRunId": {
      "type": "string"
    },
    "runs": {
      "items": {
        "$ref": "#/$defs/RunTimelineRun"
      },
      "type": "array"
    },
    "sessionId": {
      "type": "string"
    }
  },
  "required": [
    "sessionId",
    "rootRunId",
    "runs",
    "events"
  ],
  "title": "RunTimeline",
  "type": "object"
},
  RunTimelineRun: {
  "$defs": {
    "OutputContractKind": {
      "enum": [
        "debug",
        "patch",
        "review",
        "test",
        "plan",
        "custom"
      ],
      "type": "string"
    },
    "RunStatus": {
      "enum": [
        "queued",
        "running",
        "waitingForApproval",
        "completed",
        "failed",
        "budgetExceeded",
        "cancelled"
      ],
      "type": "string"
    },
    "WorktreeCleanupPolicy": {
      "enum": [
        "deleteOnSuccess",
        "deleteOnTerminal",
        "keep",
        "manual"
      ],
      "type": "string"
    },
    "WorktreeInfo": {
      "properties": {
        "branch": {
          "type": "string"
        },
        "cleanupPolicy": {
          "$ref": "#/$defs/WorktreeCleanupPolicy"
        },
        "path": {
          "type": "string"
        }
      },
      "required": [
        "path",
        "branch",
        "cleanupPolicy"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "claimedFiles": {
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "depth": {
      "format": "uint32",
      "minimum": 0,
      "type": "integer"
    },
    "endedAtMs": {
      "anyOf": [
        {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        {
          "type": "null"
        }
      ]
    },
    "outputContract": {
      "anyOf": [
        {
          "$ref": "#/$defs/OutputContractKind"
        },
        {
          "type": "null"
        }
      ]
    },
    "parentRunId": {
      "type": [
        "string",
        "null"
      ]
    },
    "recipeId": {
      "type": [
        "string",
        "null"
      ]
    },
    "runId": {
      "type": "string"
    },
    "startedAtMs": {
      "anyOf": [
        {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        {
          "type": "null"
        }
      ]
    },
    "status": {
      "$ref": "#/$defs/RunStatus"
    },
    "workspaceInfo": {
      "anyOf": [
        {
          "$ref": "#/$defs/WorktreeInfo"
        },
        {
          "type": "null"
        }
      ]
    }
  },
  "required": [
    "runId",
    "depth",
    "status"
  ],
  "title": "RunTimelineRun",
  "type": "object"
},
  RunTimelineEventKind: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "runStatus",
    "approvalRequested",
    "approvalResolved",
    "claimConflict",
    "budgetExceeded",
    "tokenUsage",
    "toolCall",
    "artifact",
    "receipt",
    "agentStream"
  ],
  "title": "RunTimelineEventKind",
  "type": "string"
},
  RunTimelineEvent: {
  "$defs": {
    "AgentToolCallOutcome": {
      "enum": [
        "completed",
        "failed",
        "cancelled"
      ],
      "type": "string"
    },
    "ApprovalDecision": {
      "enum": [
        "approved",
        "rejected"
      ],
      "type": "string"
    },
    "ApprovalScope": {
      "enum": [
        "fileWrite",
        "processExec",
        "networkAccess"
      ],
      "type": "string"
    },
    "ArtifactKind": {
      "enum": [
        "Transcript",
        "Patch",
        "FileSnapshot",
        "CommandLog"
      ],
      "type": "string"
    },
    "BudgetMetric": {
      "enum": [
        "tokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "string"
    },
    "BudgetScope": {
      "enum": [
        "run",
        "parentAggregate"
      ],
      "type": "string"
    },
    "ConflictSeverity": {
      "enum": [
        "informational",
        "warning"
      ],
      "type": "string"
    },
    "ConflictWarning": {
      "properties": {
        "conflicts": {
          "items": {
            "$ref": "#/$defs/FileClaimConflict"
          },
          "type": "array"
        },
        "requestingCapsule": {
          "type": "string"
        },
        "severity": {
          "$ref": "#/$defs/ConflictSeverity"
        }
      },
      "required": [
        "requestingCapsule",
        "severity",
        "conflicts"
      ],
      "type": "object"
    },
    "FileClaimConflict": {
      "properties": {
        "file": {
          "type": "string"
        },
        "holdingCapsule": {
          "type": "string"
        },
        "holdingKind": {
          "$ref": "#/$defs/FileClaimKind"
        }
      },
      "required": [
        "file",
        "holdingCapsule",
        "holdingKind"
      ],
      "type": "object"
    },
    "FileClaimKind": {
      "enum": [
        "write"
      ],
      "type": "string"
    },
    "ReceiptKind": {
      "enum": [
        "evidence",
        "patch",
        "testOutput",
        "reviewFinding",
        "artifact",
        "risk",
        "blocker",
        "summary"
      ],
      "type": "string"
    },
    "ReceiptState": {
      "enum": [
        "returned",
        "promoted",
        "quarantined"
      ],
      "type": "string"
    },
    "RunStatus": {
      "enum": [
        "queued",
        "running",
        "waitingForApproval",
        "completed",
        "failed",
        "budgetExceeded",
        "cancelled"
      ],
      "type": "string"
    },
    "RunTimelineEventKind": {
      "enum": [
        "runStatus",
        "approvalRequested",
        "approvalResolved",
        "claimConflict",
        "budgetExceeded",
        "tokenUsage",
        "toolCall",
        "artifact",
        "receipt",
        "agentStream"
      ],
      "type": "string"
    },
    "RunTimelineEventPayload": {
      "oneOf": [
        {
          "properties": {
            "detail": {
              "type": "string"
            },
            "kind": {
              "const": "run",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "detail"
          ],
          "type": "object"
        },
        {
          "properties": {
            "approvalId": {
              "type": "string"
            },
            "kind": {
              "const": "approvalRequested",
              "type": "string"
            },
            "scope": {
              "$ref": "#/$defs/ApprovalScope"
            }
          },
          "required": [
            "kind",
            "approvalId",
            "scope"
          ],
          "type": "object"
        },
        {
          "properties": {
            "approvalId": {
              "type": "string"
            },
            "decision": {
              "$ref": "#/$defs/ApprovalDecision"
            },
            "kind": {
              "const": "approvalResolved",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "approvalId",
            "decision"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "conflict",
              "type": "string"
            },
            "warning": {
              "$ref": "#/$defs/ConflictWarning"
            }
          },
          "required": [
            "kind",
            "warning"
          ],
          "type": "object"
        },
        {
          "properties": {
            "actual": {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            "kind": {
              "const": "budgetExceeded",
              "type": "string"
            },
            "limit": {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            "metric": {
              "$ref": "#/$defs/BudgetMetric"
            },
            "scope": {
              "$ref": "#/$defs/BudgetScope"
            }
          },
          "required": [
            "kind",
            "scope",
            "metric",
            "limit",
            "actual"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "tokenUsage",
              "type": "string"
            },
            "usage": {
              "$ref": "#/$defs/TokenUsageRecordedEvent"
            }
          },
          "required": [
            "kind",
            "usage"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "toolCall",
              "type": "string"
            },
            "outcome": {
              "anyOf": [
                {
                  "$ref": "#/$defs/AgentToolCallOutcome"
                },
                {
                  "type": "null"
                }
              ]
            },
            "toolName": {
              "type": [
                "string",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "artifactId": {
              "type": "string"
            },
            "artifactKind": {
              "$ref": "#/$defs/ArtifactKind"
            },
            "kind": {
              "const": "artifact",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "artifactId",
            "artifactKind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "receipt",
              "type": "string"
            },
            "receiptId": {
              "type": "string"
            },
            "receiptKind": {
              "$ref": "#/$defs/ReceiptKind"
            },
            "receiptState": {
              "$ref": "#/$defs/ReceiptState"
            }
          },
          "required": [
            "kind",
            "receiptId",
            "receiptKind",
            "receiptState"
          ],
          "type": "object"
        },
        {
          "properties": {
            "frameKind": {
              "type": "string"
            },
            "kind": {
              "const": "agentStream",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "frameKind"
          ],
          "type": "object"
        }
      ]
    },
    "TokenUsageRecordedEvent": {
      "properties": {
        "cachedTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "capsuleId": {
          "type": [
            "string",
            "null"
          ]
        },
        "completionTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "model": {
          "type": "string"
        },
        "promptTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "provider": {
          "type": "string"
        },
        "reasoningTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "recordedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "runId": {
          "type": "string"
        }
      },
      "required": [
        "runId",
        "promptTokens",
        "completionTokens",
        "model",
        "provider",
        "recordedAtMs"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "kind": {
      "$ref": "#/$defs/RunTimelineEventKind"
    },
    "label": {
      "type": "string"
    },
    "occurredAtMs": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    },
    "payload": {
      "$ref": "#/$defs/RunTimelineEventPayload"
    },
    "runId": {
      "type": "string"
    },
    "seq": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    },
    "status": {
      "anyOf": [
        {
          "$ref": "#/$defs/RunStatus"
        },
        {
          "type": "null"
        }
      ]
    }
  },
  "required": [
    "seq",
    "occurredAtMs",
    "runId",
    "kind",
    "label",
    "payload"
  ],
  "title": "RunTimelineEvent",
  "type": "object"
},
  RunTimelineEventPayload: {
  "$defs": {
    "AgentToolCallOutcome": {
      "enum": [
        "completed",
        "failed",
        "cancelled"
      ],
      "type": "string"
    },
    "ApprovalDecision": {
      "enum": [
        "approved",
        "rejected"
      ],
      "type": "string"
    },
    "ApprovalScope": {
      "enum": [
        "fileWrite",
        "processExec",
        "networkAccess"
      ],
      "type": "string"
    },
    "ArtifactKind": {
      "enum": [
        "Transcript",
        "Patch",
        "FileSnapshot",
        "CommandLog"
      ],
      "type": "string"
    },
    "BudgetMetric": {
      "enum": [
        "tokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "string"
    },
    "BudgetScope": {
      "enum": [
        "run",
        "parentAggregate"
      ],
      "type": "string"
    },
    "ConflictSeverity": {
      "enum": [
        "informational",
        "warning"
      ],
      "type": "string"
    },
    "ConflictWarning": {
      "properties": {
        "conflicts": {
          "items": {
            "$ref": "#/$defs/FileClaimConflict"
          },
          "type": "array"
        },
        "requestingCapsule": {
          "type": "string"
        },
        "severity": {
          "$ref": "#/$defs/ConflictSeverity"
        }
      },
      "required": [
        "requestingCapsule",
        "severity",
        "conflicts"
      ],
      "type": "object"
    },
    "FileClaimConflict": {
      "properties": {
        "file": {
          "type": "string"
        },
        "holdingCapsule": {
          "type": "string"
        },
        "holdingKind": {
          "$ref": "#/$defs/FileClaimKind"
        }
      },
      "required": [
        "file",
        "holdingCapsule",
        "holdingKind"
      ],
      "type": "object"
    },
    "FileClaimKind": {
      "enum": [
        "write"
      ],
      "type": "string"
    },
    "ReceiptKind": {
      "enum": [
        "evidence",
        "patch",
        "testOutput",
        "reviewFinding",
        "artifact",
        "risk",
        "blocker",
        "summary"
      ],
      "type": "string"
    },
    "ReceiptState": {
      "enum": [
        "returned",
        "promoted",
        "quarantined"
      ],
      "type": "string"
    },
    "TokenUsageRecordedEvent": {
      "properties": {
        "cachedTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "capsuleId": {
          "type": [
            "string",
            "null"
          ]
        },
        "completionTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "model": {
          "type": "string"
        },
        "promptTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "provider": {
          "type": "string"
        },
        "reasoningTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "recordedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "runId": {
          "type": "string"
        }
      },
      "required": [
        "runId",
        "promptTokens",
        "completionTokens",
        "model",
        "provider",
        "recordedAtMs"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "oneOf": [
    {
      "properties": {
        "detail": {
          "type": "string"
        },
        "kind": {
          "const": "run",
          "type": "string"
        }
      },
      "required": [
        "kind",
        "detail"
      ],
      "type": "object"
    },
    {
      "properties": {
        "approvalId": {
          "type": "string"
        },
        "kind": {
          "const": "approvalRequested",
          "type": "string"
        },
        "scope": {
          "$ref": "#/$defs/ApprovalScope"
        }
      },
      "required": [
        "kind",
        "approvalId",
        "scope"
      ],
      "type": "object"
    },
    {
      "properties": {
        "approvalId": {
          "type": "string"
        },
        "decision": {
          "$ref": "#/$defs/ApprovalDecision"
        },
        "kind": {
          "const": "approvalResolved",
          "type": "string"
        }
      },
      "required": [
        "kind",
        "approvalId",
        "decision"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "conflict",
          "type": "string"
        },
        "warning": {
          "$ref": "#/$defs/ConflictWarning"
        }
      },
      "required": [
        "kind",
        "warning"
      ],
      "type": "object"
    },
    {
      "properties": {
        "actual": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "kind": {
          "const": "budgetExceeded",
          "type": "string"
        },
        "limit": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "metric": {
          "$ref": "#/$defs/BudgetMetric"
        },
        "scope": {
          "$ref": "#/$defs/BudgetScope"
        }
      },
      "required": [
        "kind",
        "scope",
        "metric",
        "limit",
        "actual"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "tokenUsage",
          "type": "string"
        },
        "usage": {
          "$ref": "#/$defs/TokenUsageRecordedEvent"
        }
      },
      "required": [
        "kind",
        "usage"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "toolCall",
          "type": "string"
        },
        "outcome": {
          "anyOf": [
            {
              "$ref": "#/$defs/AgentToolCallOutcome"
            },
            {
              "type": "null"
            }
          ]
        },
        "toolName": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "kind"
      ],
      "type": "object"
    },
    {
      "properties": {
        "artifactId": {
          "type": "string"
        },
        "artifactKind": {
          "$ref": "#/$defs/ArtifactKind"
        },
        "kind": {
          "const": "artifact",
          "type": "string"
        }
      },
      "required": [
        "kind",
        "artifactId",
        "artifactKind"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "receipt",
          "type": "string"
        },
        "receiptId": {
          "type": "string"
        },
        "receiptKind": {
          "$ref": "#/$defs/ReceiptKind"
        },
        "receiptState": {
          "$ref": "#/$defs/ReceiptState"
        }
      },
      "required": [
        "kind",
        "receiptId",
        "receiptKind",
        "receiptState"
      ],
      "type": "object"
    },
    {
      "properties": {
        "frameKind": {
          "type": "string"
        },
        "kind": {
          "const": "agentStream",
          "type": "string"
        }
      },
      "required": [
        "kind",
        "frameKind"
      ],
      "type": "object"
    }
  ],
  "title": "RunTimelineEventPayload"
},
  ListSessionsQuery: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "ListSessionsQuery",
  "type": "object"
},
  ProtocolVersion: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "const": "2026-04-stage3",
  "title": "ProtocolVersion",
  "type": "string"
},
  RunId: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "string",
  "type": "string"
},
  RunHarnessKind: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "unknown",
    "native",
    "acp",
    "codexAppServer"
  ],
  "title": "RunHarnessKind",
  "type": "string"
},
  RunDetail: {
  "$defs": {
    "CapsuleResult": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "debug",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/DebugResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "patch",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/PatchResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "review",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/ReviewResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "test",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/TestResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "plan",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/PlanResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "custom",
              "type": "string"
            },
            "value": true
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        }
      ]
    },
    "ConflictSummary": {
      "properties": {
        "files": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "warningCount": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        }
      },
      "required": [
        "warningCount",
        "files"
      ],
      "type": "object"
    },
    "ContextReceipt": {
      "properties": {
        "createdAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "id": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/$defs/ReceiptKind"
        },
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "promotedAtMs": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "provenance": {
          "$ref": "#/$defs/ReceiptProvenance"
        },
        "quarantinedAtMs": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "runId": {
          "type": "string"
        },
        "sessionId": {
          "type": "string"
        },
        "state": {
          "$ref": "#/$defs/ReceiptState"
        },
        "summary": {
          "type": [
            "string",
            "null"
          ]
        },
        "title": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "sessionId",
        "runId",
        "kind",
        "provenance",
        "state",
        "createdAtMs"
      ],
      "type": "object"
    },
    "DebugResult": {
      "properties": {
        "blockers": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "confidence": {
          "maximum": 1,
          "minimum": 0,
          "type": "number"
        },
        "evidenceReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "patchReceiptId": {
          "type": [
            "string",
            "null"
          ]
        },
        "reproduced": {
          "type": "boolean"
        },
        "rootCause": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "reproduced",
        "evidenceReceiptIds",
        "confidence",
        "blockers"
      ],
      "type": "object"
    },
    "FindingSeverity": {
      "enum": [
        "low",
        "medium",
        "high",
        "critical"
      ],
      "type": "string"
    },
    "OutputContractKind": {
      "enum": [
        "debug",
        "patch",
        "review",
        "test",
        "plan",
        "custom"
      ],
      "type": "string"
    },
    "PatchResult": {
      "properties": {
        "blockers": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "passing": {
          "type": "boolean"
        },
        "patchReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "testsRunReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "touchedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        }
      },
      "required": [
        "patchReceiptIds",
        "touchedFiles",
        "testsRunReceiptIds",
        "passing",
        "blockers"
      ],
      "type": "object"
    },
    "PlanResult": {
      "properties": {
        "estimatedTotalMinutes": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "risks": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "steps": {
          "items": {
            "$ref": "#/$defs/PlanStep"
          },
          "type": "array"
        }
      },
      "required": [
        "steps",
        "risks"
      ],
      "type": "object"
    },
    "PlanStep": {
      "properties": {
        "dependsOn": {
          "items": {
            "format": "uint32",
            "minimum": 0,
            "type": "integer"
          },
          "type": "array"
        },
        "description": {
          "type": [
            "string",
            "null"
          ]
        },
        "estimatedMinutes": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "title": {
          "type": "string"
        }
      },
      "required": [
        "title",
        "dependsOn"
      ],
      "type": "object"
    },
    "ReceiptKind": {
      "enum": [
        "evidence",
        "patch",
        "testOutput",
        "reviewFinding",
        "artifact",
        "risk",
        "blocker",
        "summary"
      ],
      "type": "string"
    },
    "ReceiptProvenance": {
      "description": "Provenance shape rules:\n- artifact-derived: only `artifact_id` is set; identity = (session, run, kind, artifact_id).\n- event-derived: both `event_seq` and `agent_turn_id` are set; identity = (session, run, kind, event_seq, agent_turn_id).\n- free-form: all identifying fields are None.\n\n`stream_cursor` is descriptive metadata (e.g. for UI navigation) and may be\npresent in any shape. It is never part of the unique identity.",
      "properties": {
        "agentTurnId": {
          "type": [
            "string",
            "null"
          ]
        },
        "artifactId": {
          "type": [
            "string",
            "null"
          ]
        },
        "eventSeq": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "streamCursor": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "type": "object"
    },
    "ReceiptState": {
      "enum": [
        "returned",
        "promoted",
        "quarantined"
      ],
      "type": "string"
    },
    "ReviewFinding": {
      "properties": {
        "file": {
          "type": [
            "string",
            "null"
          ]
        },
        "line": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "message": {
          "type": "string"
        },
        "severity": {
          "$ref": "#/$defs/FindingSeverity"
        },
        "suggestion": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "severity",
        "message"
      ],
      "type": "object"
    },
    "ReviewResult": {
      "properties": {
        "findings": {
          "items": {
            "$ref": "#/$defs/ReviewFinding"
          },
          "type": "array"
        },
        "risks": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "touchedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "verdict": {
          "$ref": "#/$defs/ReviewVerdict"
        }
      },
      "required": [
        "verdict",
        "findings",
        "risks",
        "touchedFiles"
      ],
      "type": "object"
    },
    "ReviewVerdict": {
      "enum": [
        "approve",
        "requestChanges",
        "needsHuman"
      ],
      "type": "string"
    },
    "RunStatus": {
      "enum": [
        "queued",
        "running",
        "waitingForApproval",
        "completed",
        "failed",
        "budgetExceeded",
        "cancelled"
      ],
      "type": "string"
    },
    "RunSummary": {
      "properties": {
        "id": {
          "type": "string"
        },
        "objective": {
          "type": "string"
        },
        "runtimeProfileId": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/RunStatus"
        }
      },
      "required": [
        "id",
        "runtimeProfileId",
        "objective",
        "status"
      ],
      "type": "object"
    },
    "TestResult": {
      "properties": {
        "failed": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "failedTestNames": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "logReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "passed": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "skipped": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "total": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        }
      },
      "required": [
        "total",
        "passed",
        "failed",
        "skipped",
        "failedTestNames",
        "logReceiptIds"
      ],
      "type": "object"
    },
    "TokenUsageTotals": {
      "properties": {
        "cachedTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "completionTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "promptTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "reasoningTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "promptTokens",
        "completionTokens",
        "cachedTokens",
        "reasoningTokens"
      ],
      "type": "object"
    },
    "ValidationError": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "kindMismatch",
              "type": "string"
            },
            "value": {
              "properties": {
                "expected": {
                  "$ref": "#/$defs/OutputContractKind"
                },
                "got": {
                  "$ref": "#/$defs/OutputContractKind"
                }
              },
              "required": [
                "expected",
                "got"
              ],
              "type": "object"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "confidenceOutOfRange",
              "type": "string"
            },
            "value": {
              "properties": {
                "value": {
                  "type": "number"
                }
              },
              "required": [
                "value"
              ],
              "type": "object"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "invalidReceiptId",
              "type": "string"
            },
            "value": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "testCountsInconsistent",
              "type": "string"
            },
            "value": {
              "properties": {
                "sumOfParts": {
                  "format": "uint32",
                  "minimum": 0,
                  "type": "integer"
                },
                "total": {
                  "format": "uint32",
                  "minimum": 0,
                  "type": "integer"
                }
              },
              "required": [
                "total",
                "sumOfParts"
              ],
              "type": "object"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "testCountsOverflow",
              "type": "string"
            },
            "value": {
              "properties": {
                "failed": {
                  "format": "uint32",
                  "minimum": 0,
                  "type": "integer"
                },
                "passed": {
                  "format": "uint32",
                  "minimum": 0,
                  "type": "integer"
                },
                "skipped": {
                  "format": "uint32",
                  "minimum": 0,
                  "type": "integer"
                }
              },
              "required": [
                "passed",
                "failed",
                "skipped"
              ],
              "type": "object"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "planStepDependencyOutOfRange",
              "type": "string"
            },
            "value": {
              "properties": {
                "dependency": {
                  "format": "uint32",
                  "minimum": 0,
                  "type": "integer"
                },
                "stepIndex": {
                  "format": "uint32",
                  "minimum": 0,
                  "type": "integer"
                },
                "totalSteps": {
                  "format": "uint32",
                  "minimum": 0,
                  "type": "integer"
                }
              },
              "required": [
                "stepIndex",
                "dependency",
                "totalSteps"
              ],
              "type": "object"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "custom",
              "type": "string"
            },
            "value": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        }
      ]
    },
    "WorktreeCleanupPolicy": {
      "enum": [
        "deleteOnSuccess",
        "deleteOnTerminal",
        "keep",
        "manual"
      ],
      "type": "string"
    },
    "WorktreeInfo": {
      "properties": {
        "branch": {
          "type": "string"
        },
        "cleanupPolicy": {
          "$ref": "#/$defs/WorktreeCleanupPolicy"
        },
        "path": {
          "type": "string"
        }
      },
      "required": [
        "path",
        "branch",
        "cleanupPolicy"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "claimedFiles": {
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "conflictSummary": {
      "anyOf": [
        {
          "$ref": "#/$defs/ConflictSummary"
        },
        {
          "type": "null"
        }
      ]
    },
    "contractViolation": {
      "anyOf": [
        {
          "$ref": "#/$defs/ValidationError"
        },
        {
          "type": "null"
        }
      ]
    },
    "outputContract": {
      "anyOf": [
        {
          "$ref": "#/$defs/OutputContractKind"
        },
        {
          "type": "null"
        }
      ]
    },
    "parentRunId": {
      "type": [
        "string",
        "null"
      ]
    },
    "quarantineReceipt": {
      "anyOf": [
        {
          "$ref": "#/$defs/ContextReceipt"
        },
        {
          "type": "null"
        }
      ]
    },
    "recipeId": {
      "type": [
        "string",
        "null"
      ]
    },
    "result": {
      "anyOf": [
        {
          "$ref": "#/$defs/CapsuleResult"
        },
        {
          "type": "null"
        }
      ]
    },
    "summary": {
      "$ref": "#/$defs/RunSummary"
    },
    "tokenUsage": {
      "anyOf": [
        {
          "$ref": "#/$defs/TokenUsageTotals"
        },
        {
          "type": "null"
        }
      ]
    },
    "workspaceInfo": {
      "anyOf": [
        {
          "$ref": "#/$defs/WorktreeInfo"
        },
        {
          "type": "null"
        }
      ]
    }
  },
  "required": [
    "summary"
  ],
  "title": "RunDetail",
  "type": "object"
},
  RunRecord: {
  "$defs": {
    "ConflictSummary": {
      "properties": {
        "files": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "warningCount": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        }
      },
      "required": [
        "warningCount",
        "files"
      ],
      "type": "object"
    },
    "OutputContractKind": {
      "enum": [
        "debug",
        "patch",
        "review",
        "test",
        "plan",
        "custom"
      ],
      "type": "string"
    },
    "RunHarnessKind": {
      "enum": [
        "unknown",
        "native",
        "acp",
        "codexAppServer"
      ],
      "type": "string"
    },
    "RunSource": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "user",
              "type": "string"
            },
            "modelId": {
              "type": [
                "string",
                "null"
              ]
            },
            "outputContract": {
              "anyOf": [
                {
                  "$ref": "#/$defs/OutputContractKind"
                },
                {
                  "type": "null"
                }
              ]
            },
            "recipeId": {
              "type": [
                "string",
                "null"
              ]
            },
            "sandboxProfile": {
              "type": [
                "string",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "cleanupPolicy": {
              "$ref": "#/$defs/WorktreeCleanupPolicy",
              "default": "deleteOnSuccess"
            },
            "kind": {
              "const": "nativeSubagent",
              "type": "string"
            },
            "modelId": {
              "type": [
                "string",
                "null"
              ]
            },
            "outputContract": {
              "anyOf": [
                {
                  "$ref": "#/$defs/OutputContractKind"
                },
                {
                  "type": "null"
                }
              ]
            },
            "parentRunId": {
              "type": "string"
            },
            "parentTurnId": {
              "type": "string"
            },
            "plannedWriteFiles": {
              "items": {
                "type": "string"
              },
              "type": "array"
            },
            "recipeId": {
              "type": [
                "string",
                "null"
              ]
            },
            "sandboxProfile": {
              "type": [
                "string",
                "null"
              ]
            },
            "workspaceScope": {
              "$ref": "#/$defs/WorkspaceMode",
              "default": "worktreeWrite"
            }
          },
          "required": [
            "kind",
            "parentRunId",
            "parentTurnId"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "forked",
              "type": "string"
            },
            "parentEventSeq": {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            "parentRunId": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "parentRunId",
            "parentEventSeq"
          ],
          "type": "object"
        }
      ]
    },
    "RunStatus": {
      "enum": [
        "queued",
        "running",
        "waitingForApproval",
        "completed",
        "failed",
        "budgetExceeded",
        "cancelled"
      ],
      "type": "string"
    },
    "WorkspaceMode": {
      "enum": [
        "readonly",
        "workspaceWrite",
        "worktreeWrite",
        "repoWriteWithApproval",
        "remoteWorker",
        "containerized",
        "ephemeral"
      ],
      "type": "string"
    },
    "WorktreeCleanupPolicy": {
      "enum": [
        "deleteOnSuccess",
        "deleteOnTerminal",
        "keep",
        "manual"
      ],
      "type": "string"
    },
    "WorktreeInfo": {
      "properties": {
        "branch": {
          "type": "string"
        },
        "cleanupPolicy": {
          "$ref": "#/$defs/WorktreeCleanupPolicy"
        },
        "path": {
          "type": "string"
        }
      },
      "required": [
        "path",
        "branch",
        "cleanupPolicy"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "claimedFiles": {
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "conflictSummary": {
      "anyOf": [
        {
          "$ref": "#/$defs/ConflictSummary"
        },
        {
          "type": "null"
        }
      ]
    },
    "endedAtMs": {
      "anyOf": [
        {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        {
          "type": "null"
        }
      ]
    },
    "harness": {
      "$ref": "#/$defs/RunHarnessKind"
    },
    "id": {
      "type": "string"
    },
    "lastEventSeq": {
      "anyOf": [
        {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        {
          "type": "null"
        }
      ]
    },
    "objective": {
      "type": "string"
    },
    "parentRunId": {
      "type": [
        "string",
        "null"
      ]
    },
    "runtimeProfileId": {
      "type": "string"
    },
    "sessionId": {
      "type": "string"
    },
    "source": {
      "$ref": "#/$defs/RunSource"
    },
    "startedAtMs": {
      "anyOf": [
        {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        {
          "type": "null"
        }
      ]
    },
    "status": {
      "$ref": "#/$defs/RunStatus"
    },
    "workspaceInfo": {
      "anyOf": [
        {
          "$ref": "#/$defs/WorktreeInfo"
        },
        {
          "type": "null"
        }
      ]
    }
  },
  "required": [
    "id",
    "sessionId",
    "runtimeProfileId",
    "objective",
    "status",
    "harness",
    "source"
  ],
  "title": "RunRecord",
  "type": "object"
},
  RunStatus: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "queued",
    "running",
    "waitingForApproval",
    "completed",
    "failed",
    "budgetExceeded",
    "cancelled"
  ],
  "title": "RunStatus",
  "type": "string"
},
  RunSummary: {
  "$defs": {
    "RunStatus": {
      "enum": [
        "queued",
        "running",
        "waitingForApproval",
        "completed",
        "failed",
        "budgetExceeded",
        "cancelled"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "id": {
      "type": "string"
    },
    "objective": {
      "type": "string"
    },
    "runtimeProfileId": {
      "type": "string"
    },
    "status": {
      "$ref": "#/$defs/RunStatus"
    }
  },
  "required": [
    "id",
    "runtimeProfileId",
    "objective",
    "status"
  ],
  "title": "RunSummary",
  "type": "object"
},
  ResumeRunRequest: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "runId": {
      "type": "string"
    }
  },
  "required": [
    "runId"
  ],
  "title": "ResumeRunRequest",
  "type": "object"
},
  ResumeRunResult: {
  "$defs": {
    "ConflictSummary": {
      "properties": {
        "files": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "warningCount": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        }
      },
      "required": [
        "warningCount",
        "files"
      ],
      "type": "object"
    },
    "OutputContractKind": {
      "enum": [
        "debug",
        "patch",
        "review",
        "test",
        "plan",
        "custom"
      ],
      "type": "string"
    },
    "ResumeRunState": {
      "enum": [
        "live",
        "queued"
      ],
      "type": "string"
    },
    "RunHarnessKind": {
      "enum": [
        "unknown",
        "native",
        "acp",
        "codexAppServer"
      ],
      "type": "string"
    },
    "RunRecord": {
      "properties": {
        "claimedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "conflictSummary": {
          "anyOf": [
            {
              "$ref": "#/$defs/ConflictSummary"
            },
            {
              "type": "null"
            }
          ]
        },
        "endedAtMs": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "harness": {
          "$ref": "#/$defs/RunHarnessKind"
        },
        "id": {
          "type": "string"
        },
        "lastEventSeq": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "objective": {
          "type": "string"
        },
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runtimeProfileId": {
          "type": "string"
        },
        "sessionId": {
          "type": "string"
        },
        "source": {
          "$ref": "#/$defs/RunSource"
        },
        "startedAtMs": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "status": {
          "$ref": "#/$defs/RunStatus"
        },
        "workspaceInfo": {
          "anyOf": [
            {
              "$ref": "#/$defs/WorktreeInfo"
            },
            {
              "type": "null"
            }
          ]
        }
      },
      "required": [
        "id",
        "sessionId",
        "runtimeProfileId",
        "objective",
        "status",
        "harness",
        "source"
      ],
      "type": "object"
    },
    "RunSource": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "user",
              "type": "string"
            },
            "modelId": {
              "type": [
                "string",
                "null"
              ]
            },
            "outputContract": {
              "anyOf": [
                {
                  "$ref": "#/$defs/OutputContractKind"
                },
                {
                  "type": "null"
                }
              ]
            },
            "recipeId": {
              "type": [
                "string",
                "null"
              ]
            },
            "sandboxProfile": {
              "type": [
                "string",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "cleanupPolicy": {
              "$ref": "#/$defs/WorktreeCleanupPolicy",
              "default": "deleteOnSuccess"
            },
            "kind": {
              "const": "nativeSubagent",
              "type": "string"
            },
            "modelId": {
              "type": [
                "string",
                "null"
              ]
            },
            "outputContract": {
              "anyOf": [
                {
                  "$ref": "#/$defs/OutputContractKind"
                },
                {
                  "type": "null"
                }
              ]
            },
            "parentRunId": {
              "type": "string"
            },
            "parentTurnId": {
              "type": "string"
            },
            "plannedWriteFiles": {
              "items": {
                "type": "string"
              },
              "type": "array"
            },
            "recipeId": {
              "type": [
                "string",
                "null"
              ]
            },
            "sandboxProfile": {
              "type": [
                "string",
                "null"
              ]
            },
            "workspaceScope": {
              "$ref": "#/$defs/WorkspaceMode",
              "default": "worktreeWrite"
            }
          },
          "required": [
            "kind",
            "parentRunId",
            "parentTurnId"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "forked",
              "type": "string"
            },
            "parentEventSeq": {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            "parentRunId": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "parentRunId",
            "parentEventSeq"
          ],
          "type": "object"
        }
      ]
    },
    "RunStatus": {
      "enum": [
        "queued",
        "running",
        "waitingForApproval",
        "completed",
        "failed",
        "budgetExceeded",
        "cancelled"
      ],
      "type": "string"
    },
    "WorkspaceMode": {
      "enum": [
        "readonly",
        "workspaceWrite",
        "worktreeWrite",
        "repoWriteWithApproval",
        "remoteWorker",
        "containerized",
        "ephemeral"
      ],
      "type": "string"
    },
    "WorktreeCleanupPolicy": {
      "enum": [
        "deleteOnSuccess",
        "deleteOnTerminal",
        "keep",
        "manual"
      ],
      "type": "string"
    },
    "WorktreeInfo": {
      "properties": {
        "branch": {
          "type": "string"
        },
        "cleanupPolicy": {
          "$ref": "#/$defs/WorktreeCleanupPolicy"
        },
        "path": {
          "type": "string"
        }
      },
      "required": [
        "path",
        "branch",
        "cleanupPolicy"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "latestEventSeq": {
      "anyOf": [
        {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        {
          "type": "null"
        }
      ]
    },
    "run": {
      "$ref": "#/$defs/RunRecord"
    },
    "state": {
      "$ref": "#/$defs/ResumeRunState"
    }
  },
  "required": [
    "run",
    "state"
  ],
  "title": "ResumeRunResult",
  "type": "object"
},
  ResumeRunState: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "live",
    "queued"
  ],
  "title": "ResumeRunState",
  "type": "string"
},
  ForkRunRequest: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "objective": {
      "type": [
        "string",
        "null"
      ]
    },
    "parentEventSeq": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    },
    "parentRunId": {
      "type": "string"
    },
    "sessionId": {
      "type": "string"
    }
  },
  "required": [
    "sessionId",
    "parentRunId",
    "parentEventSeq"
  ],
  "title": "ForkRunRequest",
  "type": "object"
},
  ForkRunResult: {
  "$defs": {
    "ConflictSummary": {
      "properties": {
        "files": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "warningCount": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        }
      },
      "required": [
        "warningCount",
        "files"
      ],
      "type": "object"
    },
    "OutputContractKind": {
      "enum": [
        "debug",
        "patch",
        "review",
        "test",
        "plan",
        "custom"
      ],
      "type": "string"
    },
    "RunHarnessKind": {
      "enum": [
        "unknown",
        "native",
        "acp",
        "codexAppServer"
      ],
      "type": "string"
    },
    "RunRecord": {
      "properties": {
        "claimedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "conflictSummary": {
          "anyOf": [
            {
              "$ref": "#/$defs/ConflictSummary"
            },
            {
              "type": "null"
            }
          ]
        },
        "endedAtMs": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "harness": {
          "$ref": "#/$defs/RunHarnessKind"
        },
        "id": {
          "type": "string"
        },
        "lastEventSeq": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "objective": {
          "type": "string"
        },
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runtimeProfileId": {
          "type": "string"
        },
        "sessionId": {
          "type": "string"
        },
        "source": {
          "$ref": "#/$defs/RunSource"
        },
        "startedAtMs": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "status": {
          "$ref": "#/$defs/RunStatus"
        },
        "workspaceInfo": {
          "anyOf": [
            {
              "$ref": "#/$defs/WorktreeInfo"
            },
            {
              "type": "null"
            }
          ]
        }
      },
      "required": [
        "id",
        "sessionId",
        "runtimeProfileId",
        "objective",
        "status",
        "harness",
        "source"
      ],
      "type": "object"
    },
    "RunSource": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "user",
              "type": "string"
            },
            "modelId": {
              "type": [
                "string",
                "null"
              ]
            },
            "outputContract": {
              "anyOf": [
                {
                  "$ref": "#/$defs/OutputContractKind"
                },
                {
                  "type": "null"
                }
              ]
            },
            "recipeId": {
              "type": [
                "string",
                "null"
              ]
            },
            "sandboxProfile": {
              "type": [
                "string",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "cleanupPolicy": {
              "$ref": "#/$defs/WorktreeCleanupPolicy",
              "default": "deleteOnSuccess"
            },
            "kind": {
              "const": "nativeSubagent",
              "type": "string"
            },
            "modelId": {
              "type": [
                "string",
                "null"
              ]
            },
            "outputContract": {
              "anyOf": [
                {
                  "$ref": "#/$defs/OutputContractKind"
                },
                {
                  "type": "null"
                }
              ]
            },
            "parentRunId": {
              "type": "string"
            },
            "parentTurnId": {
              "type": "string"
            },
            "plannedWriteFiles": {
              "items": {
                "type": "string"
              },
              "type": "array"
            },
            "recipeId": {
              "type": [
                "string",
                "null"
              ]
            },
            "sandboxProfile": {
              "type": [
                "string",
                "null"
              ]
            },
            "workspaceScope": {
              "$ref": "#/$defs/WorkspaceMode",
              "default": "worktreeWrite"
            }
          },
          "required": [
            "kind",
            "parentRunId",
            "parentTurnId"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "forked",
              "type": "string"
            },
            "parentEventSeq": {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            "parentRunId": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "parentRunId",
            "parentEventSeq"
          ],
          "type": "object"
        }
      ]
    },
    "RunStatus": {
      "enum": [
        "queued",
        "running",
        "waitingForApproval",
        "completed",
        "failed",
        "budgetExceeded",
        "cancelled"
      ],
      "type": "string"
    },
    "WorkspaceMode": {
      "enum": [
        "readonly",
        "workspaceWrite",
        "worktreeWrite",
        "repoWriteWithApproval",
        "remoteWorker",
        "containerized",
        "ephemeral"
      ],
      "type": "string"
    },
    "WorktreeCleanupPolicy": {
      "enum": [
        "deleteOnSuccess",
        "deleteOnTerminal",
        "keep",
        "manual"
      ],
      "type": "string"
    },
    "WorktreeInfo": {
      "properties": {
        "branch": {
          "type": "string"
        },
        "cleanupPolicy": {
          "$ref": "#/$defs/WorktreeCleanupPolicy"
        },
        "path": {
          "type": "string"
        }
      },
      "required": [
        "path",
        "branch",
        "cleanupPolicy"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "run": {
      "$ref": "#/$defs/RunRecord"
    }
  },
  "required": [
    "run"
  ],
  "title": "ForkRunResult",
  "type": "object"
},
  SubscribeRunEventsRequest: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "description": "Run event subscription request after an optional durable cursor.\n\n`daemon.run.replay_events` uses this shape for finite replay batches.\n`daemon.run.subscribe_events` uses it for replay plus live splice streams.",
  "properties": {
    "afterSeq": {
      "anyOf": [
        {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        {
          "type": "null"
        }
      ]
    },
    "runId": {
      "type": "string"
    },
    "sessionId": {
      "type": "string"
    }
  },
  "required": [
    "sessionId",
    "runId"
  ],
  "title": "SubscribeRunEventsRequest",
  "type": "object"
},
  RunEventDelta: {
  "$defs": {
    "AgentStreamEvent": {
      "properties": {
        "fragmentSequence": {
          "format": "uint64",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "frame": {
          "$ref": "#/$defs/AgentStreamFrame"
        },
        "itemId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "turnId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "runId",
        "frame"
      ],
      "type": "object"
    },
    "AgentStreamFrame": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "assistantTurnStarted",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "delta": {
              "type": "string"
            },
            "kind": {
              "const": "assistantMessageDelta",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "delta"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "assistantTurnCompleted",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "input": {
              "type": "string"
            },
            "kind": {
              "const": "toolCallStarted",
              "type": "string"
            },
            "toolName": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "toolName",
            "input"
          ],
          "type": "object"
        },
        {
          "properties": {
            "delta": {
              "type": "string"
            },
            "kind": {
              "const": "toolCallProgressed",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "delta"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "toolCallCompleted",
              "type": "string"
            },
            "outcome": {
              "$ref": "#/$defs/AgentToolCallOutcome"
            }
          },
          "required": [
            "kind",
            "outcome"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "pendingStateChanged",
              "type": "string"
            },
            "state": {
              "$ref": "#/$defs/RuntimeLanePendingState"
            }
          },
          "required": [
            "kind",
            "state"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "tokenUsageUpdated",
              "type": "string"
            },
            "modelContextWindow": {
              "format": "uint64",
              "minimum": 0,
              "type": [
                "integer",
                "null"
              ]
            },
            "totalTokens": {
              "format": "uint64",
              "minimum": 0,
              "type": [
                "integer",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "AgentToolCallOutcome": {
      "enum": [
        "completed",
        "failed",
        "cancelled"
      ],
      "type": "string"
    },
    "ApprovalDecision": {
      "enum": [
        "approved",
        "rejected"
      ],
      "type": "string"
    },
    "ApprovalRequest": {
      "properties": {
        "expiresAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "id": {
          "type": "string"
        },
        "reason": {
          "type": "string"
        },
        "requestedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "runId": {
          "type": "string"
        },
        "scope": {
          "$ref": "#/$defs/ApprovalScope"
        },
        "target": {
          "$ref": "#/$defs/ApprovalTarget"
        },
        "toolCallId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "runId",
        "scope",
        "requestedAtMs",
        "expiresAtMs",
        "target",
        "reason"
      ],
      "type": "object"
    },
    "ApprovalResolutionReason": {
      "enum": [
        "user",
        "expired",
        "cancelled",
        "budgetExceeded",
        "runtimePolicy"
      ],
      "type": "string"
    },
    "ApprovalScope": {
      "enum": [
        "fileWrite",
        "processExec",
        "networkAccess"
      ],
      "type": "string"
    },
    "ApprovalTarget": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "toolCall",
              "type": "string"
            },
            "toolName": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "toolName"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "fileWrite",
              "type": "string"
            },
            "paths": {
              "items": {
                "type": "string"
              },
              "type": "array"
            }
          },
          "required": [
            "kind",
            "paths"
          ],
          "type": "object"
        },
        {
          "properties": {
            "command": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "processExec",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "host": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "networkAccess",
              "type": "string"
            },
            "protocol": {
              "type": [
                "string",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "childRunId": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "capsuleDispatch",
              "type": "string"
            },
            "workspaceScope": {
              "anyOf": [
                {
                  "$ref": "#/$defs/WorkspaceMode"
                },
                {
                  "type": "null"
                }
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "ArtifactEvent": {
      "properties": {
        "artifact": {
          "$ref": "#/$defs/ArtifactSummary"
        }
      },
      "required": [
        "artifact"
      ],
      "type": "object"
    },
    "ArtifactKind": {
      "enum": [
        "Transcript",
        "Patch",
        "FileSnapshot",
        "CommandLog"
      ],
      "type": "string"
    },
    "ArtifactSummary": {
      "properties": {
        "id": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/$defs/ArtifactKind"
        },
        "runId": {
          "type": "string"
        },
        "storagePath": {
          "type": "string"
        }
      },
      "required": [
        "id",
        "runId",
        "kind",
        "storagePath"
      ],
      "type": "object"
    },
    "BudgetBreach": {
      "properties": {
        "actual": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "limit": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "metric": {
          "$ref": "#/$defs/BudgetMetric"
        },
        "scope": {
          "$ref": "#/$defs/BudgetScope"
        }
      },
      "required": [
        "scope",
        "metric",
        "limit",
        "actual"
      ],
      "type": "object"
    },
    "BudgetEvent": {
      "oneOf": [
        {
          "properties": {
            "event": {
              "$ref": "#/$defs/BudgetExceededEvent"
            },
            "phase": {
              "const": "exceeded",
              "type": "string"
            }
          },
          "required": [
            "phase",
            "event"
          ],
          "type": "object"
        }
      ]
    },
    "BudgetExceededEvent": {
      "properties": {
        "breach": {
          "$ref": "#/$defs/BudgetBreach"
        },
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "snapshot": {
          "$ref": "#/$defs/BudgetSnapshot"
        }
      },
      "required": [
        "runId",
        "breach",
        "snapshot"
      ],
      "type": "object"
    },
    "BudgetMetric": {
      "enum": [
        "tokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "string"
    },
    "BudgetScope": {
      "enum": [
        "run",
        "parentAggregate"
      ],
      "type": "string"
    },
    "BudgetSnapshot": {
      "properties": {
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "scope": {
          "$ref": "#/$defs/BudgetScope"
        },
        "toolCalls": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "totalTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "wallClockMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "runId",
        "scope",
        "totalTokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "object"
    },
    "CapsuleResult": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "debug",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/DebugResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "patch",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/PatchResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "review",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/ReviewResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "test",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/TestResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "plan",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/PlanResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "custom",
              "type": "string"
            },
            "value": true
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        }
      ]
    },
    "ConflictEvent": {
      "oneOf": [
        {
          "properties": {
            "phase": {
              "const": "warning",
              "type": "string"
            },
            "run_id": {
              "type": "string"
            },
            "warning": {
              "$ref": "#/$defs/ConflictWarning"
            }
          },
          "required": [
            "phase",
            "run_id",
            "warning"
          ],
          "type": "object"
        }
      ]
    },
    "ConflictSeverity": {
      "enum": [
        "informational",
        "warning"
      ],
      "type": "string"
    },
    "ConflictWarning": {
      "properties": {
        "conflicts": {
          "items": {
            "$ref": "#/$defs/FileClaimConflict"
          },
          "type": "array"
        },
        "requestingCapsule": {
          "type": "string"
        },
        "severity": {
          "$ref": "#/$defs/ConflictSeverity"
        }
      },
      "required": [
        "requestingCapsule",
        "severity",
        "conflicts"
      ],
      "type": "object"
    },
    "DebugResult": {
      "properties": {
        "blockers": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "confidence": {
          "maximum": 1,
          "minimum": 0,
          "type": "number"
        },
        "evidenceReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "patchReceiptId": {
          "type": [
            "string",
            "null"
          ]
        },
        "reproduced": {
          "type": "boolean"
        },
        "rootCause": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "reproduced",
        "evidenceReceiptIds",
        "confidence",
        "blockers"
      ],
      "type": "object"
    },
    "FileClaimConflict": {
      "properties": {
        "file": {
          "type": "string"
        },
        "holdingCapsule": {
          "type": "string"
        },
        "holdingKind": {
          "$ref": "#/$defs/FileClaimKind"
        }
      },
      "required": [
        "file",
        "holdingCapsule",
        "holdingKind"
      ],
      "type": "object"
    },
    "FileClaimKind": {
      "enum": [
        "write"
      ],
      "type": "string"
    },
    "FindingSeverity": {
      "enum": [
        "low",
        "medium",
        "high",
        "critical"
      ],
      "type": "string"
    },
    "OutputContractKind": {
      "enum": [
        "debug",
        "patch",
        "review",
        "test",
        "plan",
        "custom"
      ],
      "type": "string"
    },
    "PatchResult": {
      "properties": {
        "blockers": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "passing": {
          "type": "boolean"
        },
        "patchReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "testsRunReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "touchedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        }
      },
      "required": [
        "patchReceiptIds",
        "touchedFiles",
        "testsRunReceiptIds",
        "passing",
        "blockers"
      ],
      "type": "object"
    },
    "PlanResult": {
      "properties": {
        "estimatedTotalMinutes": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "risks": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "steps": {
          "items": {
            "$ref": "#/$defs/PlanStep"
          },
          "type": "array"
        }
      },
      "required": [
        "steps",
        "risks"
      ],
      "type": "object"
    },
    "PlanStep": {
      "properties": {
        "dependsOn": {
          "items": {
            "format": "uint32",
            "minimum": 0,
            "type": "integer"
          },
          "type": "array"
        },
        "description": {
          "type": [
            "string",
            "null"
          ]
        },
        "estimatedMinutes": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "title": {
          "type": "string"
        }
      },
      "required": [
        "title",
        "dependsOn"
      ],
      "type": "object"
    },
    "PublicApprovalEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "requested",
              "type": "string"
            },
            "request": {
              "$ref": "#/$defs/ApprovalRequest"
            }
          },
          "required": [
            "phase",
            "request"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "resolved",
              "type": "string"
            },
            "resolution": {
              "$ref": "#/$defs/PublicApprovalResolution"
            }
          },
          "required": [
            "phase",
            "resolution"
          ],
          "type": "object"
        }
      ]
    },
    "PublicApprovalResolution": {
      "additionalProperties": false,
      "properties": {
        "approvalId": {
          "type": "string"
        },
        "decision": {
          "$ref": "#/$defs/ApprovalDecision"
        },
        "reason": {
          "$ref": "#/$defs/ApprovalResolutionReason"
        },
        "runId": {
          "type": "string"
        },
        "toolCallId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "approvalId",
        "runId",
        "decision",
        "reason"
      ],
      "type": "object"
    },
    "PublicContextReceipt": {
      "additionalProperties": false,
      "properties": {
        "id": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/$defs/ReceiptKind"
        },
        "provenance": {
          "$ref": "#/$defs/ReceiptProvenance"
        },
        "state": {
          "$ref": "#/$defs/ReceiptState"
        },
        "summary": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "kind",
        "state",
        "provenance"
      ],
      "type": "object"
    },
    "PublicContextReceiptEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "created",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "promoted",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "quarantined",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        }
      ]
    },
    "PublicDaemonEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "session": {
              "$ref": "#/$defs/SessionEvent"
            }
          },
          "required": [
            "session"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "run": {
              "$ref": "#/$defs/RunEvent"
            }
          },
          "required": [
            "run"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "runReconciledOnStartup": {
              "$ref": "#/$defs/RunReconciledOnStartupEvent"
            }
          },
          "required": [
            "runReconciledOnStartup"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "approval": {
              "$ref": "#/$defs/PublicApprovalEvent"
            }
          },
          "required": [
            "approval"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "artifact": {
              "$ref": "#/$defs/ArtifactEvent"
            }
          },
          "required": [
            "artifact"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "contextReceipt": {
              "$ref": "#/$defs/PublicContextReceiptEvent"
            }
          },
          "required": [
            "contextReceipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "agentStream": {
              "$ref": "#/$defs/AgentStreamEvent"
            }
          },
          "required": [
            "agentStream"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "tokenUsageRecorded": {
              "$ref": "#/$defs/TokenUsageRecordedEvent"
            }
          },
          "required": [
            "tokenUsageRecorded"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "conflict": {
              "$ref": "#/$defs/ConflictEvent"
            }
          },
          "required": [
            "conflict"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "budget": {
              "$ref": "#/$defs/BudgetEvent"
            }
          },
          "required": [
            "budget"
          ],
          "type": "object"
        }
      ]
    },
    "ReceiptKind": {
      "enum": [
        "evidence",
        "patch",
        "testOutput",
        "reviewFinding",
        "artifact",
        "risk",
        "blocker",
        "summary"
      ],
      "type": "string"
    },
    "ReceiptProvenance": {
      "description": "Provenance shape rules:\n- artifact-derived: only `artifact_id` is set; identity = (session, run, kind, artifact_id).\n- event-derived: both `event_seq` and `agent_turn_id` are set; identity = (session, run, kind, event_seq, agent_turn_id).\n- free-form: all identifying fields are None.\n\n`stream_cursor` is descriptive metadata (e.g. for UI navigation) and may be\npresent in any shape. It is never part of the unique identity.",
      "properties": {
        "agentTurnId": {
          "type": [
            "string",
            "null"
          ]
        },
        "artifactId": {
          "type": [
            "string",
            "null"
          ]
        },
        "eventSeq": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "streamCursor": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "type": "object"
    },
    "ReceiptState": {
      "enum": [
        "returned",
        "promoted",
        "quarantined"
      ],
      "type": "string"
    },
    "ReviewFinding": {
      "properties": {
        "file": {
          "type": [
            "string",
            "null"
          ]
        },
        "line": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "message": {
          "type": "string"
        },
        "severity": {
          "$ref": "#/$defs/FindingSeverity"
        },
        "suggestion": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "severity",
        "message"
      ],
      "type": "object"
    },
    "ReviewResult": {
      "properties": {
        "findings": {
          "items": {
            "$ref": "#/$defs/ReviewFinding"
          },
          "type": "array"
        },
        "risks": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "touchedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "verdict": {
          "$ref": "#/$defs/ReviewVerdict"
        }
      },
      "required": [
        "verdict",
        "findings",
        "risks",
        "touchedFiles"
      ],
      "type": "object"
    },
    "ReviewVerdict": {
      "enum": [
        "approve",
        "requestChanges",
        "needsHuman"
      ],
      "type": "string"
    },
    "RunEvent": {
      "properties": {
        "detail": {
          "type": "string"
        },
        "outputContract": {
          "anyOf": [
            {
              "$ref": "#/$defs/OutputContractKind"
            },
            {
              "type": "null"
            }
          ]
        },
        "recipeId": {
          "type": [
            "string",
            "null"
          ]
        },
        "result": {
          "anyOf": [
            {
              "$ref": "#/$defs/CapsuleResult"
            },
            {
              "type": "null"
            }
          ]
        },
        "runId": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/RunStatus"
        }
      },
      "required": [
        "runId",
        "status",
        "detail"
      ],
      "type": "object"
    },
    "RunFailureKind": {
      "enum": [
        "daemonRestartedWhileRunning"
      ],
      "type": "string"
    },
    "RunReconciledOnStartupEvent": {
      "properties": {
        "prevStatus": {
          "$ref": "#/$defs/RunStatus"
        },
        "reason": {
          "$ref": "#/$defs/RunFailureKind"
        },
        "runId": {
          "type": "string"
        }
      },
      "required": [
        "runId",
        "prevStatus",
        "reason"
      ],
      "type": "object"
    },
    "RunStatus": {
      "enum": [
        "queued",
        "running",
        "waitingForApproval",
        "completed",
        "failed",
        "budgetExceeded",
        "cancelled"
      ],
      "type": "string"
    },
    "RuntimeLanePendingState": {
      "enum": [
        "queued",
        "waitingForApproval",
        "waitingForInput"
      ],
      "type": "string"
    },
    "SessionEvent": {
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/SessionStatus"
        }
      },
      "required": [
        "sessionId",
        "status"
      ],
      "type": "object"
    },
    "SessionStatus": {
      "enum": [
        "idle",
        "running",
        "paused",
        "failed",
        "completed"
      ],
      "type": "string"
    },
    "TestResult": {
      "properties": {
        "failed": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "failedTestNames": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "logReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "passed": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "skipped": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "total": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        }
      },
      "required": [
        "total",
        "passed",
        "failed",
        "skipped",
        "failedTestNames",
        "logReceiptIds"
      ],
      "type": "object"
    },
    "TokenUsageRecordedEvent": {
      "properties": {
        "cachedTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "capsuleId": {
          "type": [
            "string",
            "null"
          ]
        },
        "completionTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "model": {
          "type": "string"
        },
        "promptTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "provider": {
          "type": "string"
        },
        "reasoningTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "recordedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "runId": {
          "type": "string"
        }
      },
      "required": [
        "runId",
        "promptTokens",
        "completionTokens",
        "model",
        "provider",
        "recordedAtMs"
      ],
      "type": "object"
    },
    "WorkspaceMode": {
      "enum": [
        "readonly",
        "workspaceWrite",
        "worktreeWrite",
        "repoWriteWithApproval",
        "remoteWorker",
        "containerized",
        "ephemeral"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "description": "One run event delta returned by replay or live splice.\n\nThe sequence is the persisted daemon-event sequence, so clients can dedupe\nreplay and live deliveries with one cursor.",
  "properties": {
    "event": {
      "$ref": "#/$defs/PublicDaemonEvent"
    },
    "seq": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    }
  },
  "required": [
    "seq",
    "event"
  ],
  "title": "RunEventDelta",
  "type": "object"
},
  RunEventStreamError: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "lagged",
    "historyGap"
  ],
  "title": "RunEventStreamError",
  "type": "string"
},
  RunEventStreamPayload: {
  "$defs": {
    "AgentStreamEvent": {
      "properties": {
        "fragmentSequence": {
          "format": "uint64",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "frame": {
          "$ref": "#/$defs/AgentStreamFrame"
        },
        "itemId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "turnId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "runId",
        "frame"
      ],
      "type": "object"
    },
    "AgentStreamFrame": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "assistantTurnStarted",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "delta": {
              "type": "string"
            },
            "kind": {
              "const": "assistantMessageDelta",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "delta"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "assistantTurnCompleted",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "input": {
              "type": "string"
            },
            "kind": {
              "const": "toolCallStarted",
              "type": "string"
            },
            "toolName": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "toolName",
            "input"
          ],
          "type": "object"
        },
        {
          "properties": {
            "delta": {
              "type": "string"
            },
            "kind": {
              "const": "toolCallProgressed",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "delta"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "toolCallCompleted",
              "type": "string"
            },
            "outcome": {
              "$ref": "#/$defs/AgentToolCallOutcome"
            }
          },
          "required": [
            "kind",
            "outcome"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "pendingStateChanged",
              "type": "string"
            },
            "state": {
              "$ref": "#/$defs/RuntimeLanePendingState"
            }
          },
          "required": [
            "kind",
            "state"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "tokenUsageUpdated",
              "type": "string"
            },
            "modelContextWindow": {
              "format": "uint64",
              "minimum": 0,
              "type": [
                "integer",
                "null"
              ]
            },
            "totalTokens": {
              "format": "uint64",
              "minimum": 0,
              "type": [
                "integer",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "AgentToolCallOutcome": {
      "enum": [
        "completed",
        "failed",
        "cancelled"
      ],
      "type": "string"
    },
    "ApprovalDecision": {
      "enum": [
        "approved",
        "rejected"
      ],
      "type": "string"
    },
    "ApprovalRequest": {
      "properties": {
        "expiresAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "id": {
          "type": "string"
        },
        "reason": {
          "type": "string"
        },
        "requestedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "runId": {
          "type": "string"
        },
        "scope": {
          "$ref": "#/$defs/ApprovalScope"
        },
        "target": {
          "$ref": "#/$defs/ApprovalTarget"
        },
        "toolCallId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "runId",
        "scope",
        "requestedAtMs",
        "expiresAtMs",
        "target",
        "reason"
      ],
      "type": "object"
    },
    "ApprovalResolutionReason": {
      "enum": [
        "user",
        "expired",
        "cancelled",
        "budgetExceeded",
        "runtimePolicy"
      ],
      "type": "string"
    },
    "ApprovalScope": {
      "enum": [
        "fileWrite",
        "processExec",
        "networkAccess"
      ],
      "type": "string"
    },
    "ApprovalTarget": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "toolCall",
              "type": "string"
            },
            "toolName": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "toolName"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "fileWrite",
              "type": "string"
            },
            "paths": {
              "items": {
                "type": "string"
              },
              "type": "array"
            }
          },
          "required": [
            "kind",
            "paths"
          ],
          "type": "object"
        },
        {
          "properties": {
            "command": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "processExec",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "host": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "networkAccess",
              "type": "string"
            },
            "protocol": {
              "type": [
                "string",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "childRunId": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "capsuleDispatch",
              "type": "string"
            },
            "workspaceScope": {
              "anyOf": [
                {
                  "$ref": "#/$defs/WorkspaceMode"
                },
                {
                  "type": "null"
                }
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "ArtifactEvent": {
      "properties": {
        "artifact": {
          "$ref": "#/$defs/ArtifactSummary"
        }
      },
      "required": [
        "artifact"
      ],
      "type": "object"
    },
    "ArtifactKind": {
      "enum": [
        "Transcript",
        "Patch",
        "FileSnapshot",
        "CommandLog"
      ],
      "type": "string"
    },
    "ArtifactSummary": {
      "properties": {
        "id": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/$defs/ArtifactKind"
        },
        "runId": {
          "type": "string"
        },
        "storagePath": {
          "type": "string"
        }
      },
      "required": [
        "id",
        "runId",
        "kind",
        "storagePath"
      ],
      "type": "object"
    },
    "BudgetBreach": {
      "properties": {
        "actual": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "limit": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "metric": {
          "$ref": "#/$defs/BudgetMetric"
        },
        "scope": {
          "$ref": "#/$defs/BudgetScope"
        }
      },
      "required": [
        "scope",
        "metric",
        "limit",
        "actual"
      ],
      "type": "object"
    },
    "BudgetEvent": {
      "oneOf": [
        {
          "properties": {
            "event": {
              "$ref": "#/$defs/BudgetExceededEvent"
            },
            "phase": {
              "const": "exceeded",
              "type": "string"
            }
          },
          "required": [
            "phase",
            "event"
          ],
          "type": "object"
        }
      ]
    },
    "BudgetExceededEvent": {
      "properties": {
        "breach": {
          "$ref": "#/$defs/BudgetBreach"
        },
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "snapshot": {
          "$ref": "#/$defs/BudgetSnapshot"
        }
      },
      "required": [
        "runId",
        "breach",
        "snapshot"
      ],
      "type": "object"
    },
    "BudgetMetric": {
      "enum": [
        "tokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "string"
    },
    "BudgetScope": {
      "enum": [
        "run",
        "parentAggregate"
      ],
      "type": "string"
    },
    "BudgetSnapshot": {
      "properties": {
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "scope": {
          "$ref": "#/$defs/BudgetScope"
        },
        "toolCalls": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "totalTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "wallClockMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "runId",
        "scope",
        "totalTokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "object"
    },
    "CapsuleResult": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "debug",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/DebugResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "patch",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/PatchResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "review",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/ReviewResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "test",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/TestResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "plan",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/PlanResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "custom",
              "type": "string"
            },
            "value": true
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        }
      ]
    },
    "ConflictEvent": {
      "oneOf": [
        {
          "properties": {
            "phase": {
              "const": "warning",
              "type": "string"
            },
            "run_id": {
              "type": "string"
            },
            "warning": {
              "$ref": "#/$defs/ConflictWarning"
            }
          },
          "required": [
            "phase",
            "run_id",
            "warning"
          ],
          "type": "object"
        }
      ]
    },
    "ConflictSeverity": {
      "enum": [
        "informational",
        "warning"
      ],
      "type": "string"
    },
    "ConflictWarning": {
      "properties": {
        "conflicts": {
          "items": {
            "$ref": "#/$defs/FileClaimConflict"
          },
          "type": "array"
        },
        "requestingCapsule": {
          "type": "string"
        },
        "severity": {
          "$ref": "#/$defs/ConflictSeverity"
        }
      },
      "required": [
        "requestingCapsule",
        "severity",
        "conflicts"
      ],
      "type": "object"
    },
    "DebugResult": {
      "properties": {
        "blockers": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "confidence": {
          "maximum": 1,
          "minimum": 0,
          "type": "number"
        },
        "evidenceReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "patchReceiptId": {
          "type": [
            "string",
            "null"
          ]
        },
        "reproduced": {
          "type": "boolean"
        },
        "rootCause": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "reproduced",
        "evidenceReceiptIds",
        "confidence",
        "blockers"
      ],
      "type": "object"
    },
    "FileClaimConflict": {
      "properties": {
        "file": {
          "type": "string"
        },
        "holdingCapsule": {
          "type": "string"
        },
        "holdingKind": {
          "$ref": "#/$defs/FileClaimKind"
        }
      },
      "required": [
        "file",
        "holdingCapsule",
        "holdingKind"
      ],
      "type": "object"
    },
    "FileClaimKind": {
      "enum": [
        "write"
      ],
      "type": "string"
    },
    "FindingSeverity": {
      "enum": [
        "low",
        "medium",
        "high",
        "critical"
      ],
      "type": "string"
    },
    "OutputContractKind": {
      "enum": [
        "debug",
        "patch",
        "review",
        "test",
        "plan",
        "custom"
      ],
      "type": "string"
    },
    "PatchResult": {
      "properties": {
        "blockers": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "passing": {
          "type": "boolean"
        },
        "patchReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "testsRunReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "touchedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        }
      },
      "required": [
        "patchReceiptIds",
        "touchedFiles",
        "testsRunReceiptIds",
        "passing",
        "blockers"
      ],
      "type": "object"
    },
    "PlanResult": {
      "properties": {
        "estimatedTotalMinutes": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "risks": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "steps": {
          "items": {
            "$ref": "#/$defs/PlanStep"
          },
          "type": "array"
        }
      },
      "required": [
        "steps",
        "risks"
      ],
      "type": "object"
    },
    "PlanStep": {
      "properties": {
        "dependsOn": {
          "items": {
            "format": "uint32",
            "minimum": 0,
            "type": "integer"
          },
          "type": "array"
        },
        "description": {
          "type": [
            "string",
            "null"
          ]
        },
        "estimatedMinutes": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "title": {
          "type": "string"
        }
      },
      "required": [
        "title",
        "dependsOn"
      ],
      "type": "object"
    },
    "PublicApprovalEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "requested",
              "type": "string"
            },
            "request": {
              "$ref": "#/$defs/ApprovalRequest"
            }
          },
          "required": [
            "phase",
            "request"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "resolved",
              "type": "string"
            },
            "resolution": {
              "$ref": "#/$defs/PublicApprovalResolution"
            }
          },
          "required": [
            "phase",
            "resolution"
          ],
          "type": "object"
        }
      ]
    },
    "PublicApprovalResolution": {
      "additionalProperties": false,
      "properties": {
        "approvalId": {
          "type": "string"
        },
        "decision": {
          "$ref": "#/$defs/ApprovalDecision"
        },
        "reason": {
          "$ref": "#/$defs/ApprovalResolutionReason"
        },
        "runId": {
          "type": "string"
        },
        "toolCallId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "approvalId",
        "runId",
        "decision",
        "reason"
      ],
      "type": "object"
    },
    "PublicContextReceipt": {
      "additionalProperties": false,
      "properties": {
        "id": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/$defs/ReceiptKind"
        },
        "provenance": {
          "$ref": "#/$defs/ReceiptProvenance"
        },
        "state": {
          "$ref": "#/$defs/ReceiptState"
        },
        "summary": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "kind",
        "state",
        "provenance"
      ],
      "type": "object"
    },
    "PublicContextReceiptEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "created",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "promoted",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "quarantined",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        }
      ]
    },
    "PublicDaemonEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "session": {
              "$ref": "#/$defs/SessionEvent"
            }
          },
          "required": [
            "session"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "run": {
              "$ref": "#/$defs/RunEvent"
            }
          },
          "required": [
            "run"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "runReconciledOnStartup": {
              "$ref": "#/$defs/RunReconciledOnStartupEvent"
            }
          },
          "required": [
            "runReconciledOnStartup"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "approval": {
              "$ref": "#/$defs/PublicApprovalEvent"
            }
          },
          "required": [
            "approval"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "artifact": {
              "$ref": "#/$defs/ArtifactEvent"
            }
          },
          "required": [
            "artifact"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "contextReceipt": {
              "$ref": "#/$defs/PublicContextReceiptEvent"
            }
          },
          "required": [
            "contextReceipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "agentStream": {
              "$ref": "#/$defs/AgentStreamEvent"
            }
          },
          "required": [
            "agentStream"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "tokenUsageRecorded": {
              "$ref": "#/$defs/TokenUsageRecordedEvent"
            }
          },
          "required": [
            "tokenUsageRecorded"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "conflict": {
              "$ref": "#/$defs/ConflictEvent"
            }
          },
          "required": [
            "conflict"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "budget": {
              "$ref": "#/$defs/BudgetEvent"
            }
          },
          "required": [
            "budget"
          ],
          "type": "object"
        }
      ]
    },
    "ReceiptKind": {
      "enum": [
        "evidence",
        "patch",
        "testOutput",
        "reviewFinding",
        "artifact",
        "risk",
        "blocker",
        "summary"
      ],
      "type": "string"
    },
    "ReceiptProvenance": {
      "description": "Provenance shape rules:\n- artifact-derived: only `artifact_id` is set; identity = (session, run, kind, artifact_id).\n- event-derived: both `event_seq` and `agent_turn_id` are set; identity = (session, run, kind, event_seq, agent_turn_id).\n- free-form: all identifying fields are None.\n\n`stream_cursor` is descriptive metadata (e.g. for UI navigation) and may be\npresent in any shape. It is never part of the unique identity.",
      "properties": {
        "agentTurnId": {
          "type": [
            "string",
            "null"
          ]
        },
        "artifactId": {
          "type": [
            "string",
            "null"
          ]
        },
        "eventSeq": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "streamCursor": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "type": "object"
    },
    "ReceiptState": {
      "enum": [
        "returned",
        "promoted",
        "quarantined"
      ],
      "type": "string"
    },
    "ReviewFinding": {
      "properties": {
        "file": {
          "type": [
            "string",
            "null"
          ]
        },
        "line": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "message": {
          "type": "string"
        },
        "severity": {
          "$ref": "#/$defs/FindingSeverity"
        },
        "suggestion": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "severity",
        "message"
      ],
      "type": "object"
    },
    "ReviewResult": {
      "properties": {
        "findings": {
          "items": {
            "$ref": "#/$defs/ReviewFinding"
          },
          "type": "array"
        },
        "risks": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "touchedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "verdict": {
          "$ref": "#/$defs/ReviewVerdict"
        }
      },
      "required": [
        "verdict",
        "findings",
        "risks",
        "touchedFiles"
      ],
      "type": "object"
    },
    "ReviewVerdict": {
      "enum": [
        "approve",
        "requestChanges",
        "needsHuman"
      ],
      "type": "string"
    },
    "RunEvent": {
      "properties": {
        "detail": {
          "type": "string"
        },
        "outputContract": {
          "anyOf": [
            {
              "$ref": "#/$defs/OutputContractKind"
            },
            {
              "type": "null"
            }
          ]
        },
        "recipeId": {
          "type": [
            "string",
            "null"
          ]
        },
        "result": {
          "anyOf": [
            {
              "$ref": "#/$defs/CapsuleResult"
            },
            {
              "type": "null"
            }
          ]
        },
        "runId": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/RunStatus"
        }
      },
      "required": [
        "runId",
        "status",
        "detail"
      ],
      "type": "object"
    },
    "RunEventDelta": {
      "description": "One run event delta returned by replay or live splice.\n\nThe sequence is the persisted daemon-event sequence, so clients can dedupe\nreplay and live deliveries with one cursor.",
      "properties": {
        "event": {
          "$ref": "#/$defs/PublicDaemonEvent"
        },
        "seq": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "seq",
        "event"
      ],
      "type": "object"
    },
    "RunEventStreamError": {
      "enum": [
        "lagged",
        "historyGap"
      ],
      "type": "string"
    },
    "RunFailureKind": {
      "enum": [
        "daemonRestartedWhileRunning"
      ],
      "type": "string"
    },
    "RunReconciledOnStartupEvent": {
      "properties": {
        "prevStatus": {
          "$ref": "#/$defs/RunStatus"
        },
        "reason": {
          "$ref": "#/$defs/RunFailureKind"
        },
        "runId": {
          "type": "string"
        }
      },
      "required": [
        "runId",
        "prevStatus",
        "reason"
      ],
      "type": "object"
    },
    "RunStatus": {
      "enum": [
        "queued",
        "running",
        "waitingForApproval",
        "completed",
        "failed",
        "budgetExceeded",
        "cancelled"
      ],
      "type": "string"
    },
    "RuntimeLanePendingState": {
      "enum": [
        "queued",
        "waitingForApproval",
        "waitingForInput"
      ],
      "type": "string"
    },
    "SessionEvent": {
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/SessionStatus"
        }
      },
      "required": [
        "sessionId",
        "status"
      ],
      "type": "object"
    },
    "SessionStatus": {
      "enum": [
        "idle",
        "running",
        "paused",
        "failed",
        "completed"
      ],
      "type": "string"
    },
    "TestResult": {
      "properties": {
        "failed": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "failedTestNames": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "logReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "passed": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "skipped": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "total": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        }
      },
      "required": [
        "total",
        "passed",
        "failed",
        "skipped",
        "failedTestNames",
        "logReceiptIds"
      ],
      "type": "object"
    },
    "TokenUsageRecordedEvent": {
      "properties": {
        "cachedTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "capsuleId": {
          "type": [
            "string",
            "null"
          ]
        },
        "completionTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "model": {
          "type": "string"
        },
        "promptTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "provider": {
          "type": "string"
        },
        "reasoningTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "recordedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "runId": {
          "type": "string"
        }
      },
      "required": [
        "runId",
        "promptTokens",
        "completionTokens",
        "model",
        "provider",
        "recordedAtMs"
      ],
      "type": "object"
    },
    "WorkspaceMode": {
      "enum": [
        "readonly",
        "workspaceWrite",
        "worktreeWrite",
        "repoWriteWithApproval",
        "remoteWorker",
        "containerized",
        "ephemeral"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "oneOf": [
    {
      "properties": {
        "delta": {
          "$ref": "#/$defs/RunEventDelta"
        },
        "kind": {
          "const": "delta",
          "type": "string"
        }
      },
      "required": [
        "kind",
        "delta"
      ],
      "type": "object"
    },
    {
      "properties": {
        "error": {
          "$ref": "#/$defs/RunEventStreamError"
        },
        "kind": {
          "const": "error",
          "type": "string"
        }
      },
      "required": [
        "kind",
        "error"
      ],
      "type": "object"
    }
  ],
  "title": "RunEventStreamPayload"
},
  RunEventStreamItem: {
  "$defs": {
    "AgentStreamEvent": {
      "properties": {
        "fragmentSequence": {
          "format": "uint64",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "frame": {
          "$ref": "#/$defs/AgentStreamFrame"
        },
        "itemId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "turnId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "runId",
        "frame"
      ],
      "type": "object"
    },
    "AgentStreamFrame": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "assistantTurnStarted",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "delta": {
              "type": "string"
            },
            "kind": {
              "const": "assistantMessageDelta",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "delta"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "assistantTurnCompleted",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "input": {
              "type": "string"
            },
            "kind": {
              "const": "toolCallStarted",
              "type": "string"
            },
            "toolName": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "toolName",
            "input"
          ],
          "type": "object"
        },
        {
          "properties": {
            "delta": {
              "type": "string"
            },
            "kind": {
              "const": "toolCallProgressed",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "delta"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "toolCallCompleted",
              "type": "string"
            },
            "outcome": {
              "$ref": "#/$defs/AgentToolCallOutcome"
            }
          },
          "required": [
            "kind",
            "outcome"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "pendingStateChanged",
              "type": "string"
            },
            "state": {
              "$ref": "#/$defs/RuntimeLanePendingState"
            }
          },
          "required": [
            "kind",
            "state"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "tokenUsageUpdated",
              "type": "string"
            },
            "modelContextWindow": {
              "format": "uint64",
              "minimum": 0,
              "type": [
                "integer",
                "null"
              ]
            },
            "totalTokens": {
              "format": "uint64",
              "minimum": 0,
              "type": [
                "integer",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "AgentToolCallOutcome": {
      "enum": [
        "completed",
        "failed",
        "cancelled"
      ],
      "type": "string"
    },
    "ApprovalDecision": {
      "enum": [
        "approved",
        "rejected"
      ],
      "type": "string"
    },
    "ApprovalRequest": {
      "properties": {
        "expiresAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "id": {
          "type": "string"
        },
        "reason": {
          "type": "string"
        },
        "requestedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "runId": {
          "type": "string"
        },
        "scope": {
          "$ref": "#/$defs/ApprovalScope"
        },
        "target": {
          "$ref": "#/$defs/ApprovalTarget"
        },
        "toolCallId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "runId",
        "scope",
        "requestedAtMs",
        "expiresAtMs",
        "target",
        "reason"
      ],
      "type": "object"
    },
    "ApprovalResolutionReason": {
      "enum": [
        "user",
        "expired",
        "cancelled",
        "budgetExceeded",
        "runtimePolicy"
      ],
      "type": "string"
    },
    "ApprovalScope": {
      "enum": [
        "fileWrite",
        "processExec",
        "networkAccess"
      ],
      "type": "string"
    },
    "ApprovalTarget": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "toolCall",
              "type": "string"
            },
            "toolName": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "toolName"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "fileWrite",
              "type": "string"
            },
            "paths": {
              "items": {
                "type": "string"
              },
              "type": "array"
            }
          },
          "required": [
            "kind",
            "paths"
          ],
          "type": "object"
        },
        {
          "properties": {
            "command": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "processExec",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "host": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "networkAccess",
              "type": "string"
            },
            "protocol": {
              "type": [
                "string",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "childRunId": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "capsuleDispatch",
              "type": "string"
            },
            "workspaceScope": {
              "anyOf": [
                {
                  "$ref": "#/$defs/WorkspaceMode"
                },
                {
                  "type": "null"
                }
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "ArtifactEvent": {
      "properties": {
        "artifact": {
          "$ref": "#/$defs/ArtifactSummary"
        }
      },
      "required": [
        "artifact"
      ],
      "type": "object"
    },
    "ArtifactKind": {
      "enum": [
        "Transcript",
        "Patch",
        "FileSnapshot",
        "CommandLog"
      ],
      "type": "string"
    },
    "ArtifactSummary": {
      "properties": {
        "id": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/$defs/ArtifactKind"
        },
        "runId": {
          "type": "string"
        },
        "storagePath": {
          "type": "string"
        }
      },
      "required": [
        "id",
        "runId",
        "kind",
        "storagePath"
      ],
      "type": "object"
    },
    "BudgetBreach": {
      "properties": {
        "actual": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "limit": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "metric": {
          "$ref": "#/$defs/BudgetMetric"
        },
        "scope": {
          "$ref": "#/$defs/BudgetScope"
        }
      },
      "required": [
        "scope",
        "metric",
        "limit",
        "actual"
      ],
      "type": "object"
    },
    "BudgetEvent": {
      "oneOf": [
        {
          "properties": {
            "event": {
              "$ref": "#/$defs/BudgetExceededEvent"
            },
            "phase": {
              "const": "exceeded",
              "type": "string"
            }
          },
          "required": [
            "phase",
            "event"
          ],
          "type": "object"
        }
      ]
    },
    "BudgetExceededEvent": {
      "properties": {
        "breach": {
          "$ref": "#/$defs/BudgetBreach"
        },
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "snapshot": {
          "$ref": "#/$defs/BudgetSnapshot"
        }
      },
      "required": [
        "runId",
        "breach",
        "snapshot"
      ],
      "type": "object"
    },
    "BudgetMetric": {
      "enum": [
        "tokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "string"
    },
    "BudgetScope": {
      "enum": [
        "run",
        "parentAggregate"
      ],
      "type": "string"
    },
    "BudgetSnapshot": {
      "properties": {
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "scope": {
          "$ref": "#/$defs/BudgetScope"
        },
        "toolCalls": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "totalTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "wallClockMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "runId",
        "scope",
        "totalTokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "object"
    },
    "CapsuleResult": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "debug",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/DebugResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "patch",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/PatchResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "review",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/ReviewResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "test",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/TestResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "plan",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/PlanResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "custom",
              "type": "string"
            },
            "value": true
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        }
      ]
    },
    "ConflictEvent": {
      "oneOf": [
        {
          "properties": {
            "phase": {
              "const": "warning",
              "type": "string"
            },
            "run_id": {
              "type": "string"
            },
            "warning": {
              "$ref": "#/$defs/ConflictWarning"
            }
          },
          "required": [
            "phase",
            "run_id",
            "warning"
          ],
          "type": "object"
        }
      ]
    },
    "ConflictSeverity": {
      "enum": [
        "informational",
        "warning"
      ],
      "type": "string"
    },
    "ConflictWarning": {
      "properties": {
        "conflicts": {
          "items": {
            "$ref": "#/$defs/FileClaimConflict"
          },
          "type": "array"
        },
        "requestingCapsule": {
          "type": "string"
        },
        "severity": {
          "$ref": "#/$defs/ConflictSeverity"
        }
      },
      "required": [
        "requestingCapsule",
        "severity",
        "conflicts"
      ],
      "type": "object"
    },
    "DebugResult": {
      "properties": {
        "blockers": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "confidence": {
          "maximum": 1,
          "minimum": 0,
          "type": "number"
        },
        "evidenceReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "patchReceiptId": {
          "type": [
            "string",
            "null"
          ]
        },
        "reproduced": {
          "type": "boolean"
        },
        "rootCause": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "reproduced",
        "evidenceReceiptIds",
        "confidence",
        "blockers"
      ],
      "type": "object"
    },
    "FileClaimConflict": {
      "properties": {
        "file": {
          "type": "string"
        },
        "holdingCapsule": {
          "type": "string"
        },
        "holdingKind": {
          "$ref": "#/$defs/FileClaimKind"
        }
      },
      "required": [
        "file",
        "holdingCapsule",
        "holdingKind"
      ],
      "type": "object"
    },
    "FileClaimKind": {
      "enum": [
        "write"
      ],
      "type": "string"
    },
    "FindingSeverity": {
      "enum": [
        "low",
        "medium",
        "high",
        "critical"
      ],
      "type": "string"
    },
    "OutputContractKind": {
      "enum": [
        "debug",
        "patch",
        "review",
        "test",
        "plan",
        "custom"
      ],
      "type": "string"
    },
    "PatchResult": {
      "properties": {
        "blockers": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "passing": {
          "type": "boolean"
        },
        "patchReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "testsRunReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "touchedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        }
      },
      "required": [
        "patchReceiptIds",
        "touchedFiles",
        "testsRunReceiptIds",
        "passing",
        "blockers"
      ],
      "type": "object"
    },
    "PlanResult": {
      "properties": {
        "estimatedTotalMinutes": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "risks": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "steps": {
          "items": {
            "$ref": "#/$defs/PlanStep"
          },
          "type": "array"
        }
      },
      "required": [
        "steps",
        "risks"
      ],
      "type": "object"
    },
    "PlanStep": {
      "properties": {
        "dependsOn": {
          "items": {
            "format": "uint32",
            "minimum": 0,
            "type": "integer"
          },
          "type": "array"
        },
        "description": {
          "type": [
            "string",
            "null"
          ]
        },
        "estimatedMinutes": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "title": {
          "type": "string"
        }
      },
      "required": [
        "title",
        "dependsOn"
      ],
      "type": "object"
    },
    "PublicApprovalEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "requested",
              "type": "string"
            },
            "request": {
              "$ref": "#/$defs/ApprovalRequest"
            }
          },
          "required": [
            "phase",
            "request"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "resolved",
              "type": "string"
            },
            "resolution": {
              "$ref": "#/$defs/PublicApprovalResolution"
            }
          },
          "required": [
            "phase",
            "resolution"
          ],
          "type": "object"
        }
      ]
    },
    "PublicApprovalResolution": {
      "additionalProperties": false,
      "properties": {
        "approvalId": {
          "type": "string"
        },
        "decision": {
          "$ref": "#/$defs/ApprovalDecision"
        },
        "reason": {
          "$ref": "#/$defs/ApprovalResolutionReason"
        },
        "runId": {
          "type": "string"
        },
        "toolCallId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "approvalId",
        "runId",
        "decision",
        "reason"
      ],
      "type": "object"
    },
    "PublicContextReceipt": {
      "additionalProperties": false,
      "properties": {
        "id": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/$defs/ReceiptKind"
        },
        "provenance": {
          "$ref": "#/$defs/ReceiptProvenance"
        },
        "state": {
          "$ref": "#/$defs/ReceiptState"
        },
        "summary": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "kind",
        "state",
        "provenance"
      ],
      "type": "object"
    },
    "PublicContextReceiptEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "created",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "promoted",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "quarantined",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        }
      ]
    },
    "PublicDaemonEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "session": {
              "$ref": "#/$defs/SessionEvent"
            }
          },
          "required": [
            "session"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "run": {
              "$ref": "#/$defs/RunEvent"
            }
          },
          "required": [
            "run"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "runReconciledOnStartup": {
              "$ref": "#/$defs/RunReconciledOnStartupEvent"
            }
          },
          "required": [
            "runReconciledOnStartup"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "approval": {
              "$ref": "#/$defs/PublicApprovalEvent"
            }
          },
          "required": [
            "approval"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "artifact": {
              "$ref": "#/$defs/ArtifactEvent"
            }
          },
          "required": [
            "artifact"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "contextReceipt": {
              "$ref": "#/$defs/PublicContextReceiptEvent"
            }
          },
          "required": [
            "contextReceipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "agentStream": {
              "$ref": "#/$defs/AgentStreamEvent"
            }
          },
          "required": [
            "agentStream"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "tokenUsageRecorded": {
              "$ref": "#/$defs/TokenUsageRecordedEvent"
            }
          },
          "required": [
            "tokenUsageRecorded"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "conflict": {
              "$ref": "#/$defs/ConflictEvent"
            }
          },
          "required": [
            "conflict"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "budget": {
              "$ref": "#/$defs/BudgetEvent"
            }
          },
          "required": [
            "budget"
          ],
          "type": "object"
        }
      ]
    },
    "ReceiptKind": {
      "enum": [
        "evidence",
        "patch",
        "testOutput",
        "reviewFinding",
        "artifact",
        "risk",
        "blocker",
        "summary"
      ],
      "type": "string"
    },
    "ReceiptProvenance": {
      "description": "Provenance shape rules:\n- artifact-derived: only `artifact_id` is set; identity = (session, run, kind, artifact_id).\n- event-derived: both `event_seq` and `agent_turn_id` are set; identity = (session, run, kind, event_seq, agent_turn_id).\n- free-form: all identifying fields are None.\n\n`stream_cursor` is descriptive metadata (e.g. for UI navigation) and may be\npresent in any shape. It is never part of the unique identity.",
      "properties": {
        "agentTurnId": {
          "type": [
            "string",
            "null"
          ]
        },
        "artifactId": {
          "type": [
            "string",
            "null"
          ]
        },
        "eventSeq": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "streamCursor": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "type": "object"
    },
    "ReceiptState": {
      "enum": [
        "returned",
        "promoted",
        "quarantined"
      ],
      "type": "string"
    },
    "ReviewFinding": {
      "properties": {
        "file": {
          "type": [
            "string",
            "null"
          ]
        },
        "line": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "message": {
          "type": "string"
        },
        "severity": {
          "$ref": "#/$defs/FindingSeverity"
        },
        "suggestion": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "severity",
        "message"
      ],
      "type": "object"
    },
    "ReviewResult": {
      "properties": {
        "findings": {
          "items": {
            "$ref": "#/$defs/ReviewFinding"
          },
          "type": "array"
        },
        "risks": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "touchedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "verdict": {
          "$ref": "#/$defs/ReviewVerdict"
        }
      },
      "required": [
        "verdict",
        "findings",
        "risks",
        "touchedFiles"
      ],
      "type": "object"
    },
    "ReviewVerdict": {
      "enum": [
        "approve",
        "requestChanges",
        "needsHuman"
      ],
      "type": "string"
    },
    "RunEvent": {
      "properties": {
        "detail": {
          "type": "string"
        },
        "outputContract": {
          "anyOf": [
            {
              "$ref": "#/$defs/OutputContractKind"
            },
            {
              "type": "null"
            }
          ]
        },
        "recipeId": {
          "type": [
            "string",
            "null"
          ]
        },
        "result": {
          "anyOf": [
            {
              "$ref": "#/$defs/CapsuleResult"
            },
            {
              "type": "null"
            }
          ]
        },
        "runId": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/RunStatus"
        }
      },
      "required": [
        "runId",
        "status",
        "detail"
      ],
      "type": "object"
    },
    "RunEventDelta": {
      "description": "One run event delta returned by replay or live splice.\n\nThe sequence is the persisted daemon-event sequence, so clients can dedupe\nreplay and live deliveries with one cursor.",
      "properties": {
        "event": {
          "$ref": "#/$defs/PublicDaemonEvent"
        },
        "seq": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "seq",
        "event"
      ],
      "type": "object"
    },
    "RunEventStreamError": {
      "enum": [
        "lagged",
        "historyGap"
      ],
      "type": "string"
    },
    "RunEventStreamPayload": {
      "oneOf": [
        {
          "properties": {
            "delta": {
              "$ref": "#/$defs/RunEventDelta"
            },
            "kind": {
              "const": "delta",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "delta"
          ],
          "type": "object"
        },
        {
          "properties": {
            "error": {
              "$ref": "#/$defs/RunEventStreamError"
            },
            "kind": {
              "const": "error",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "error"
          ],
          "type": "object"
        }
      ]
    },
    "RunFailureKind": {
      "enum": [
        "daemonRestartedWhileRunning"
      ],
      "type": "string"
    },
    "RunReconciledOnStartupEvent": {
      "properties": {
        "prevStatus": {
          "$ref": "#/$defs/RunStatus"
        },
        "reason": {
          "$ref": "#/$defs/RunFailureKind"
        },
        "runId": {
          "type": "string"
        }
      },
      "required": [
        "runId",
        "prevStatus",
        "reason"
      ],
      "type": "object"
    },
    "RunStatus": {
      "enum": [
        "queued",
        "running",
        "waitingForApproval",
        "completed",
        "failed",
        "budgetExceeded",
        "cancelled"
      ],
      "type": "string"
    },
    "RuntimeLanePendingState": {
      "enum": [
        "queued",
        "waitingForApproval",
        "waitingForInput"
      ],
      "type": "string"
    },
    "SessionEvent": {
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/SessionStatus"
        }
      },
      "required": [
        "sessionId",
        "status"
      ],
      "type": "object"
    },
    "SessionStatus": {
      "enum": [
        "idle",
        "running",
        "paused",
        "failed",
        "completed"
      ],
      "type": "string"
    },
    "TestResult": {
      "properties": {
        "failed": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "failedTestNames": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "logReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "passed": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "skipped": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "total": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        }
      },
      "required": [
        "total",
        "passed",
        "failed",
        "skipped",
        "failedTestNames",
        "logReceiptIds"
      ],
      "type": "object"
    },
    "TokenUsageRecordedEvent": {
      "properties": {
        "cachedTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "capsuleId": {
          "type": [
            "string",
            "null"
          ]
        },
        "completionTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "model": {
          "type": "string"
        },
        "promptTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "provider": {
          "type": "string"
        },
        "reasoningTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "recordedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "runId": {
          "type": "string"
        }
      },
      "required": [
        "runId",
        "promptTokens",
        "completionTokens",
        "model",
        "provider",
        "recordedAtMs"
      ],
      "type": "object"
    },
    "WorkspaceMode": {
      "enum": [
        "readonly",
        "workspaceWrite",
        "worktreeWrite",
        "repoWriteWithApproval",
        "remoteWorker",
        "containerized",
        "ephemeral"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "payload": {
      "$ref": "#/$defs/RunEventStreamPayload"
    },
    "runId": {
      "type": "string"
    }
  },
  "required": [
    "runId",
    "payload"
  ],
  "title": "RunEventStreamItem",
  "type": "object"
},
  SubscribeRunEventsResult: {
  "$defs": {
    "AgentStreamEvent": {
      "properties": {
        "fragmentSequence": {
          "format": "uint64",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "frame": {
          "$ref": "#/$defs/AgentStreamFrame"
        },
        "itemId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "turnId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "runId",
        "frame"
      ],
      "type": "object"
    },
    "AgentStreamFrame": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "assistantTurnStarted",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "delta": {
              "type": "string"
            },
            "kind": {
              "const": "assistantMessageDelta",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "delta"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "assistantTurnCompleted",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "input": {
              "type": "string"
            },
            "kind": {
              "const": "toolCallStarted",
              "type": "string"
            },
            "toolName": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "toolName",
            "input"
          ],
          "type": "object"
        },
        {
          "properties": {
            "delta": {
              "type": "string"
            },
            "kind": {
              "const": "toolCallProgressed",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "delta"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "toolCallCompleted",
              "type": "string"
            },
            "outcome": {
              "$ref": "#/$defs/AgentToolCallOutcome"
            }
          },
          "required": [
            "kind",
            "outcome"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "pendingStateChanged",
              "type": "string"
            },
            "state": {
              "$ref": "#/$defs/RuntimeLanePendingState"
            }
          },
          "required": [
            "kind",
            "state"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "tokenUsageUpdated",
              "type": "string"
            },
            "modelContextWindow": {
              "format": "uint64",
              "minimum": 0,
              "type": [
                "integer",
                "null"
              ]
            },
            "totalTokens": {
              "format": "uint64",
              "minimum": 0,
              "type": [
                "integer",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "AgentToolCallOutcome": {
      "enum": [
        "completed",
        "failed",
        "cancelled"
      ],
      "type": "string"
    },
    "ApprovalDecision": {
      "enum": [
        "approved",
        "rejected"
      ],
      "type": "string"
    },
    "ApprovalRequest": {
      "properties": {
        "expiresAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "id": {
          "type": "string"
        },
        "reason": {
          "type": "string"
        },
        "requestedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "runId": {
          "type": "string"
        },
        "scope": {
          "$ref": "#/$defs/ApprovalScope"
        },
        "target": {
          "$ref": "#/$defs/ApprovalTarget"
        },
        "toolCallId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "runId",
        "scope",
        "requestedAtMs",
        "expiresAtMs",
        "target",
        "reason"
      ],
      "type": "object"
    },
    "ApprovalResolutionReason": {
      "enum": [
        "user",
        "expired",
        "cancelled",
        "budgetExceeded",
        "runtimePolicy"
      ],
      "type": "string"
    },
    "ApprovalScope": {
      "enum": [
        "fileWrite",
        "processExec",
        "networkAccess"
      ],
      "type": "string"
    },
    "ApprovalTarget": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "toolCall",
              "type": "string"
            },
            "toolName": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "toolName"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "fileWrite",
              "type": "string"
            },
            "paths": {
              "items": {
                "type": "string"
              },
              "type": "array"
            }
          },
          "required": [
            "kind",
            "paths"
          ],
          "type": "object"
        },
        {
          "properties": {
            "command": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "processExec",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "host": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "networkAccess",
              "type": "string"
            },
            "protocol": {
              "type": [
                "string",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "childRunId": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "capsuleDispatch",
              "type": "string"
            },
            "workspaceScope": {
              "anyOf": [
                {
                  "$ref": "#/$defs/WorkspaceMode"
                },
                {
                  "type": "null"
                }
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "ArtifactEvent": {
      "properties": {
        "artifact": {
          "$ref": "#/$defs/ArtifactSummary"
        }
      },
      "required": [
        "artifact"
      ],
      "type": "object"
    },
    "ArtifactKind": {
      "enum": [
        "Transcript",
        "Patch",
        "FileSnapshot",
        "CommandLog"
      ],
      "type": "string"
    },
    "ArtifactSummary": {
      "properties": {
        "id": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/$defs/ArtifactKind"
        },
        "runId": {
          "type": "string"
        },
        "storagePath": {
          "type": "string"
        }
      },
      "required": [
        "id",
        "runId",
        "kind",
        "storagePath"
      ],
      "type": "object"
    },
    "BudgetBreach": {
      "properties": {
        "actual": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "limit": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "metric": {
          "$ref": "#/$defs/BudgetMetric"
        },
        "scope": {
          "$ref": "#/$defs/BudgetScope"
        }
      },
      "required": [
        "scope",
        "metric",
        "limit",
        "actual"
      ],
      "type": "object"
    },
    "BudgetEvent": {
      "oneOf": [
        {
          "properties": {
            "event": {
              "$ref": "#/$defs/BudgetExceededEvent"
            },
            "phase": {
              "const": "exceeded",
              "type": "string"
            }
          },
          "required": [
            "phase",
            "event"
          ],
          "type": "object"
        }
      ]
    },
    "BudgetExceededEvent": {
      "properties": {
        "breach": {
          "$ref": "#/$defs/BudgetBreach"
        },
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "snapshot": {
          "$ref": "#/$defs/BudgetSnapshot"
        }
      },
      "required": [
        "runId",
        "breach",
        "snapshot"
      ],
      "type": "object"
    },
    "BudgetMetric": {
      "enum": [
        "tokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "string"
    },
    "BudgetScope": {
      "enum": [
        "run",
        "parentAggregate"
      ],
      "type": "string"
    },
    "BudgetSnapshot": {
      "properties": {
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "scope": {
          "$ref": "#/$defs/BudgetScope"
        },
        "toolCalls": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "totalTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "wallClockMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "runId",
        "scope",
        "totalTokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "object"
    },
    "CapsuleResult": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "debug",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/DebugResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "patch",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/PatchResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "review",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/ReviewResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "test",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/TestResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "plan",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/PlanResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "custom",
              "type": "string"
            },
            "value": true
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        }
      ]
    },
    "ConflictEvent": {
      "oneOf": [
        {
          "properties": {
            "phase": {
              "const": "warning",
              "type": "string"
            },
            "run_id": {
              "type": "string"
            },
            "warning": {
              "$ref": "#/$defs/ConflictWarning"
            }
          },
          "required": [
            "phase",
            "run_id",
            "warning"
          ],
          "type": "object"
        }
      ]
    },
    "ConflictSeverity": {
      "enum": [
        "informational",
        "warning"
      ],
      "type": "string"
    },
    "ConflictWarning": {
      "properties": {
        "conflicts": {
          "items": {
            "$ref": "#/$defs/FileClaimConflict"
          },
          "type": "array"
        },
        "requestingCapsule": {
          "type": "string"
        },
        "severity": {
          "$ref": "#/$defs/ConflictSeverity"
        }
      },
      "required": [
        "requestingCapsule",
        "severity",
        "conflicts"
      ],
      "type": "object"
    },
    "DebugResult": {
      "properties": {
        "blockers": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "confidence": {
          "maximum": 1,
          "minimum": 0,
          "type": "number"
        },
        "evidenceReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "patchReceiptId": {
          "type": [
            "string",
            "null"
          ]
        },
        "reproduced": {
          "type": "boolean"
        },
        "rootCause": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "reproduced",
        "evidenceReceiptIds",
        "confidence",
        "blockers"
      ],
      "type": "object"
    },
    "FileClaimConflict": {
      "properties": {
        "file": {
          "type": "string"
        },
        "holdingCapsule": {
          "type": "string"
        },
        "holdingKind": {
          "$ref": "#/$defs/FileClaimKind"
        }
      },
      "required": [
        "file",
        "holdingCapsule",
        "holdingKind"
      ],
      "type": "object"
    },
    "FileClaimKind": {
      "enum": [
        "write"
      ],
      "type": "string"
    },
    "FindingSeverity": {
      "enum": [
        "low",
        "medium",
        "high",
        "critical"
      ],
      "type": "string"
    },
    "OutputContractKind": {
      "enum": [
        "debug",
        "patch",
        "review",
        "test",
        "plan",
        "custom"
      ],
      "type": "string"
    },
    "PatchResult": {
      "properties": {
        "blockers": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "passing": {
          "type": "boolean"
        },
        "patchReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "testsRunReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "touchedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        }
      },
      "required": [
        "patchReceiptIds",
        "touchedFiles",
        "testsRunReceiptIds",
        "passing",
        "blockers"
      ],
      "type": "object"
    },
    "PlanResult": {
      "properties": {
        "estimatedTotalMinutes": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "risks": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "steps": {
          "items": {
            "$ref": "#/$defs/PlanStep"
          },
          "type": "array"
        }
      },
      "required": [
        "steps",
        "risks"
      ],
      "type": "object"
    },
    "PlanStep": {
      "properties": {
        "dependsOn": {
          "items": {
            "format": "uint32",
            "minimum": 0,
            "type": "integer"
          },
          "type": "array"
        },
        "description": {
          "type": [
            "string",
            "null"
          ]
        },
        "estimatedMinutes": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "title": {
          "type": "string"
        }
      },
      "required": [
        "title",
        "dependsOn"
      ],
      "type": "object"
    },
    "PublicApprovalEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "requested",
              "type": "string"
            },
            "request": {
              "$ref": "#/$defs/ApprovalRequest"
            }
          },
          "required": [
            "phase",
            "request"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "resolved",
              "type": "string"
            },
            "resolution": {
              "$ref": "#/$defs/PublicApprovalResolution"
            }
          },
          "required": [
            "phase",
            "resolution"
          ],
          "type": "object"
        }
      ]
    },
    "PublicApprovalResolution": {
      "additionalProperties": false,
      "properties": {
        "approvalId": {
          "type": "string"
        },
        "decision": {
          "$ref": "#/$defs/ApprovalDecision"
        },
        "reason": {
          "$ref": "#/$defs/ApprovalResolutionReason"
        },
        "runId": {
          "type": "string"
        },
        "toolCallId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "approvalId",
        "runId",
        "decision",
        "reason"
      ],
      "type": "object"
    },
    "PublicContextReceipt": {
      "additionalProperties": false,
      "properties": {
        "id": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/$defs/ReceiptKind"
        },
        "provenance": {
          "$ref": "#/$defs/ReceiptProvenance"
        },
        "state": {
          "$ref": "#/$defs/ReceiptState"
        },
        "summary": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "kind",
        "state",
        "provenance"
      ],
      "type": "object"
    },
    "PublicContextReceiptEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "created",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "promoted",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "quarantined",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        }
      ]
    },
    "PublicDaemonEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "session": {
              "$ref": "#/$defs/SessionEvent"
            }
          },
          "required": [
            "session"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "run": {
              "$ref": "#/$defs/RunEvent"
            }
          },
          "required": [
            "run"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "runReconciledOnStartup": {
              "$ref": "#/$defs/RunReconciledOnStartupEvent"
            }
          },
          "required": [
            "runReconciledOnStartup"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "approval": {
              "$ref": "#/$defs/PublicApprovalEvent"
            }
          },
          "required": [
            "approval"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "artifact": {
              "$ref": "#/$defs/ArtifactEvent"
            }
          },
          "required": [
            "artifact"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "contextReceipt": {
              "$ref": "#/$defs/PublicContextReceiptEvent"
            }
          },
          "required": [
            "contextReceipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "agentStream": {
              "$ref": "#/$defs/AgentStreamEvent"
            }
          },
          "required": [
            "agentStream"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "tokenUsageRecorded": {
              "$ref": "#/$defs/TokenUsageRecordedEvent"
            }
          },
          "required": [
            "tokenUsageRecorded"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "conflict": {
              "$ref": "#/$defs/ConflictEvent"
            }
          },
          "required": [
            "conflict"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "budget": {
              "$ref": "#/$defs/BudgetEvent"
            }
          },
          "required": [
            "budget"
          ],
          "type": "object"
        }
      ]
    },
    "ReceiptKind": {
      "enum": [
        "evidence",
        "patch",
        "testOutput",
        "reviewFinding",
        "artifact",
        "risk",
        "blocker",
        "summary"
      ],
      "type": "string"
    },
    "ReceiptProvenance": {
      "description": "Provenance shape rules:\n- artifact-derived: only `artifact_id` is set; identity = (session, run, kind, artifact_id).\n- event-derived: both `event_seq` and `agent_turn_id` are set; identity = (session, run, kind, event_seq, agent_turn_id).\n- free-form: all identifying fields are None.\n\n`stream_cursor` is descriptive metadata (e.g. for UI navigation) and may be\npresent in any shape. It is never part of the unique identity.",
      "properties": {
        "agentTurnId": {
          "type": [
            "string",
            "null"
          ]
        },
        "artifactId": {
          "type": [
            "string",
            "null"
          ]
        },
        "eventSeq": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "streamCursor": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "type": "object"
    },
    "ReceiptState": {
      "enum": [
        "returned",
        "promoted",
        "quarantined"
      ],
      "type": "string"
    },
    "ReviewFinding": {
      "properties": {
        "file": {
          "type": [
            "string",
            "null"
          ]
        },
        "line": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "message": {
          "type": "string"
        },
        "severity": {
          "$ref": "#/$defs/FindingSeverity"
        },
        "suggestion": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "severity",
        "message"
      ],
      "type": "object"
    },
    "ReviewResult": {
      "properties": {
        "findings": {
          "items": {
            "$ref": "#/$defs/ReviewFinding"
          },
          "type": "array"
        },
        "risks": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "touchedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "verdict": {
          "$ref": "#/$defs/ReviewVerdict"
        }
      },
      "required": [
        "verdict",
        "findings",
        "risks",
        "touchedFiles"
      ],
      "type": "object"
    },
    "ReviewVerdict": {
      "enum": [
        "approve",
        "requestChanges",
        "needsHuman"
      ],
      "type": "string"
    },
    "RunEvent": {
      "properties": {
        "detail": {
          "type": "string"
        },
        "outputContract": {
          "anyOf": [
            {
              "$ref": "#/$defs/OutputContractKind"
            },
            {
              "type": "null"
            }
          ]
        },
        "recipeId": {
          "type": [
            "string",
            "null"
          ]
        },
        "result": {
          "anyOf": [
            {
              "$ref": "#/$defs/CapsuleResult"
            },
            {
              "type": "null"
            }
          ]
        },
        "runId": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/RunStatus"
        }
      },
      "required": [
        "runId",
        "status",
        "detail"
      ],
      "type": "object"
    },
    "RunEventDelta": {
      "description": "One run event delta returned by replay or live splice.\n\nThe sequence is the persisted daemon-event sequence, so clients can dedupe\nreplay and live deliveries with one cursor.",
      "properties": {
        "event": {
          "$ref": "#/$defs/PublicDaemonEvent"
        },
        "seq": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "seq",
        "event"
      ],
      "type": "object"
    },
    "RunFailureKind": {
      "enum": [
        "daemonRestartedWhileRunning"
      ],
      "type": "string"
    },
    "RunReconciledOnStartupEvent": {
      "properties": {
        "prevStatus": {
          "$ref": "#/$defs/RunStatus"
        },
        "reason": {
          "$ref": "#/$defs/RunFailureKind"
        },
        "runId": {
          "type": "string"
        }
      },
      "required": [
        "runId",
        "prevStatus",
        "reason"
      ],
      "type": "object"
    },
    "RunStatus": {
      "enum": [
        "queued",
        "running",
        "waitingForApproval",
        "completed",
        "failed",
        "budgetExceeded",
        "cancelled"
      ],
      "type": "string"
    },
    "RuntimeLanePendingState": {
      "enum": [
        "queued",
        "waitingForApproval",
        "waitingForInput"
      ],
      "type": "string"
    },
    "SessionEvent": {
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/SessionStatus"
        }
      },
      "required": [
        "sessionId",
        "status"
      ],
      "type": "object"
    },
    "SessionStatus": {
      "enum": [
        "idle",
        "running",
        "paused",
        "failed",
        "completed"
      ],
      "type": "string"
    },
    "TestResult": {
      "properties": {
        "failed": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "failedTestNames": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "logReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "passed": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "skipped": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "total": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        }
      },
      "required": [
        "total",
        "passed",
        "failed",
        "skipped",
        "failedTestNames",
        "logReceiptIds"
      ],
      "type": "object"
    },
    "TokenUsageRecordedEvent": {
      "properties": {
        "cachedTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "capsuleId": {
          "type": [
            "string",
            "null"
          ]
        },
        "completionTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "model": {
          "type": "string"
        },
        "promptTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "provider": {
          "type": "string"
        },
        "reasoningTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "recordedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "runId": {
          "type": "string"
        }
      },
      "required": [
        "runId",
        "promptTokens",
        "completionTokens",
        "model",
        "provider",
        "recordedAtMs"
      ],
      "type": "object"
    },
    "WorkspaceMode": {
      "enum": [
        "readonly",
        "workspaceWrite",
        "worktreeWrite",
        "repoWriteWithApproval",
        "remoteWorker",
        "containerized",
        "ephemeral"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "description": "Replay-only result for durable run events.\n\nThe event list is a finite historical batch. No live stream is opened by this\nresult; live splice uses `RunEventDelta` as its stream item.",
  "properties": {
    "events": {
      "items": {
        "$ref": "#/$defs/RunEventDelta"
      },
      "type": "array"
    },
    "latestEventSeq": {
      "anyOf": [
        {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        {
          "type": "null"
        }
      ]
    }
  },
  "required": [
    "events"
  ],
  "title": "SubscribeRunEventsResult",
  "type": "object"
},
  SessionAuthority: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "string",
  "type": "string"
},
  SessionId: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "string",
  "type": "string"
},
  SessionOverview: {
  "$defs": {
    "AgentStreamEvent": {
      "properties": {
        "fragmentSequence": {
          "format": "uint64",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "frame": {
          "$ref": "#/$defs/AgentStreamFrame"
        },
        "itemId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "turnId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "runId",
        "frame"
      ],
      "type": "object"
    },
    "AgentStreamFrame": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "assistantTurnStarted",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "delta": {
              "type": "string"
            },
            "kind": {
              "const": "assistantMessageDelta",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "delta"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "assistantTurnCompleted",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "input": {
              "type": "string"
            },
            "kind": {
              "const": "toolCallStarted",
              "type": "string"
            },
            "toolName": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "toolName",
            "input"
          ],
          "type": "object"
        },
        {
          "properties": {
            "delta": {
              "type": "string"
            },
            "kind": {
              "const": "toolCallProgressed",
              "type": "string"
            }
          },
          "required": [
            "kind",
            "delta"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "toolCallCompleted",
              "type": "string"
            },
            "outcome": {
              "$ref": "#/$defs/AgentToolCallOutcome"
            }
          },
          "required": [
            "kind",
            "outcome"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "pendingStateChanged",
              "type": "string"
            },
            "state": {
              "$ref": "#/$defs/RuntimeLanePendingState"
            }
          },
          "required": [
            "kind",
            "state"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "tokenUsageUpdated",
              "type": "string"
            },
            "modelContextWindow": {
              "format": "uint64",
              "minimum": 0,
              "type": [
                "integer",
                "null"
              ]
            },
            "totalTokens": {
              "format": "uint64",
              "minimum": 0,
              "type": [
                "integer",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "AgentToolCallOutcome": {
      "enum": [
        "completed",
        "failed",
        "cancelled"
      ],
      "type": "string"
    },
    "ApprovalAttentionState": {
      "enum": [
        "idle",
        "pending"
      ],
      "type": "string"
    },
    "ApprovalDecision": {
      "enum": [
        "approved",
        "rejected"
      ],
      "type": "string"
    },
    "ApprovalRequest": {
      "properties": {
        "expiresAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "id": {
          "type": "string"
        },
        "reason": {
          "type": "string"
        },
        "requestedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "runId": {
          "type": "string"
        },
        "scope": {
          "$ref": "#/$defs/ApprovalScope"
        },
        "target": {
          "$ref": "#/$defs/ApprovalTarget"
        },
        "toolCallId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "runId",
        "scope",
        "requestedAtMs",
        "expiresAtMs",
        "target",
        "reason"
      ],
      "type": "object"
    },
    "ApprovalResolutionReason": {
      "enum": [
        "user",
        "expired",
        "cancelled",
        "budgetExceeded",
        "runtimePolicy"
      ],
      "type": "string"
    },
    "ApprovalScope": {
      "enum": [
        "fileWrite",
        "processExec",
        "networkAccess"
      ],
      "type": "string"
    },
    "ApprovalTarget": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "toolCall",
              "type": "string"
            },
            "toolName": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "toolName"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "fileWrite",
              "type": "string"
            },
            "paths": {
              "items": {
                "type": "string"
              },
              "type": "array"
            }
          },
          "required": [
            "kind",
            "paths"
          ],
          "type": "object"
        },
        {
          "properties": {
            "command": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "processExec",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "host": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "networkAccess",
              "type": "string"
            },
            "protocol": {
              "type": [
                "string",
                "null"
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        },
        {
          "properties": {
            "childRunId": {
              "type": [
                "string",
                "null"
              ]
            },
            "kind": {
              "const": "capsuleDispatch",
              "type": "string"
            },
            "workspaceScope": {
              "anyOf": [
                {
                  "$ref": "#/$defs/WorkspaceMode"
                },
                {
                  "type": "null"
                }
              ]
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "ArtifactEvent": {
      "properties": {
        "artifact": {
          "$ref": "#/$defs/ArtifactSummary"
        }
      },
      "required": [
        "artifact"
      ],
      "type": "object"
    },
    "ArtifactKind": {
      "enum": [
        "Transcript",
        "Patch",
        "FileSnapshot",
        "CommandLog"
      ],
      "type": "string"
    },
    "ArtifactSummary": {
      "properties": {
        "id": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/$defs/ArtifactKind"
        },
        "runId": {
          "type": "string"
        },
        "storagePath": {
          "type": "string"
        }
      },
      "required": [
        "id",
        "runId",
        "kind",
        "storagePath"
      ],
      "type": "object"
    },
    "BudgetBreach": {
      "properties": {
        "actual": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "limit": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "metric": {
          "$ref": "#/$defs/BudgetMetric"
        },
        "scope": {
          "$ref": "#/$defs/BudgetScope"
        }
      },
      "required": [
        "scope",
        "metric",
        "limit",
        "actual"
      ],
      "type": "object"
    },
    "BudgetEvent": {
      "oneOf": [
        {
          "properties": {
            "event": {
              "$ref": "#/$defs/BudgetExceededEvent"
            },
            "phase": {
              "const": "exceeded",
              "type": "string"
            }
          },
          "required": [
            "phase",
            "event"
          ],
          "type": "object"
        }
      ]
    },
    "BudgetExceededEvent": {
      "properties": {
        "breach": {
          "$ref": "#/$defs/BudgetBreach"
        },
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "snapshot": {
          "$ref": "#/$defs/BudgetSnapshot"
        }
      },
      "required": [
        "runId",
        "breach",
        "snapshot"
      ],
      "type": "object"
    },
    "BudgetMetric": {
      "enum": [
        "tokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "string"
    },
    "BudgetScope": {
      "enum": [
        "run",
        "parentAggregate"
      ],
      "type": "string"
    },
    "BudgetSnapshot": {
      "properties": {
        "parentRunId": {
          "type": [
            "string",
            "null"
          ]
        },
        "runId": {
          "type": "string"
        },
        "scope": {
          "$ref": "#/$defs/BudgetScope"
        },
        "toolCalls": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "totalTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "wallClockMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "runId",
        "scope",
        "totalTokens",
        "wallClockMs",
        "toolCalls"
      ],
      "type": "object"
    },
    "CapsuleResult": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "debug",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/DebugResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "patch",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/PatchResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "review",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/ReviewResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "test",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/TestResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "plan",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/PlanResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "custom",
              "type": "string"
            },
            "value": true
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        }
      ]
    },
    "ConflictEvent": {
      "oneOf": [
        {
          "properties": {
            "phase": {
              "const": "warning",
              "type": "string"
            },
            "run_id": {
              "type": "string"
            },
            "warning": {
              "$ref": "#/$defs/ConflictWarning"
            }
          },
          "required": [
            "phase",
            "run_id",
            "warning"
          ],
          "type": "object"
        }
      ]
    },
    "ConflictSeverity": {
      "enum": [
        "informational",
        "warning"
      ],
      "type": "string"
    },
    "ConflictWarning": {
      "properties": {
        "conflicts": {
          "items": {
            "$ref": "#/$defs/FileClaimConflict"
          },
          "type": "array"
        },
        "requestingCapsule": {
          "type": "string"
        },
        "severity": {
          "$ref": "#/$defs/ConflictSeverity"
        }
      },
      "required": [
        "requestingCapsule",
        "severity",
        "conflicts"
      ],
      "type": "object"
    },
    "DebugResult": {
      "properties": {
        "blockers": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "confidence": {
          "maximum": 1,
          "minimum": 0,
          "type": "number"
        },
        "evidenceReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "patchReceiptId": {
          "type": [
            "string",
            "null"
          ]
        },
        "reproduced": {
          "type": "boolean"
        },
        "rootCause": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "reproduced",
        "evidenceReceiptIds",
        "confidence",
        "blockers"
      ],
      "type": "object"
    },
    "FileClaimConflict": {
      "properties": {
        "file": {
          "type": "string"
        },
        "holdingCapsule": {
          "type": "string"
        },
        "holdingKind": {
          "$ref": "#/$defs/FileClaimKind"
        }
      },
      "required": [
        "file",
        "holdingCapsule",
        "holdingKind"
      ],
      "type": "object"
    },
    "FileClaimKind": {
      "enum": [
        "write"
      ],
      "type": "string"
    },
    "FindingSeverity": {
      "enum": [
        "low",
        "medium",
        "high",
        "critical"
      ],
      "type": "string"
    },
    "OutputContractKind": {
      "enum": [
        "debug",
        "patch",
        "review",
        "test",
        "plan",
        "custom"
      ],
      "type": "string"
    },
    "PatchResult": {
      "properties": {
        "blockers": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "passing": {
          "type": "boolean"
        },
        "patchReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "testsRunReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "touchedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        }
      },
      "required": [
        "patchReceiptIds",
        "touchedFiles",
        "testsRunReceiptIds",
        "passing",
        "blockers"
      ],
      "type": "object"
    },
    "PlanResult": {
      "properties": {
        "estimatedTotalMinutes": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "risks": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "steps": {
          "items": {
            "$ref": "#/$defs/PlanStep"
          },
          "type": "array"
        }
      },
      "required": [
        "steps",
        "risks"
      ],
      "type": "object"
    },
    "PlanStep": {
      "properties": {
        "dependsOn": {
          "items": {
            "format": "uint32",
            "minimum": 0,
            "type": "integer"
          },
          "type": "array"
        },
        "description": {
          "type": [
            "string",
            "null"
          ]
        },
        "estimatedMinutes": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "title": {
          "type": "string"
        }
      },
      "required": [
        "title",
        "dependsOn"
      ],
      "type": "object"
    },
    "PublicApprovalEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "requested",
              "type": "string"
            },
            "request": {
              "$ref": "#/$defs/ApprovalRequest"
            }
          },
          "required": [
            "phase",
            "request"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "resolved",
              "type": "string"
            },
            "resolution": {
              "$ref": "#/$defs/PublicApprovalResolution"
            }
          },
          "required": [
            "phase",
            "resolution"
          ],
          "type": "object"
        }
      ]
    },
    "PublicApprovalResolution": {
      "additionalProperties": false,
      "properties": {
        "approvalId": {
          "type": "string"
        },
        "decision": {
          "$ref": "#/$defs/ApprovalDecision"
        },
        "reason": {
          "$ref": "#/$defs/ApprovalResolutionReason"
        },
        "runId": {
          "type": "string"
        },
        "toolCallId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "approvalId",
        "runId",
        "decision",
        "reason"
      ],
      "type": "object"
    },
    "PublicContextReceipt": {
      "additionalProperties": false,
      "properties": {
        "id": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/$defs/ReceiptKind"
        },
        "provenance": {
          "$ref": "#/$defs/ReceiptProvenance"
        },
        "state": {
          "$ref": "#/$defs/ReceiptState"
        },
        "summary": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "kind",
        "state",
        "provenance"
      ],
      "type": "object"
    },
    "PublicContextReceiptEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "created",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "promoted",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "phase": {
              "const": "quarantined",
              "type": "string"
            },
            "receipt": {
              "$ref": "#/$defs/PublicContextReceipt"
            }
          },
          "required": [
            "phase",
            "receipt"
          ],
          "type": "object"
        }
      ]
    },
    "PublicDaemonEvent": {
      "oneOf": [
        {
          "additionalProperties": false,
          "properties": {
            "session": {
              "$ref": "#/$defs/SessionEvent"
            }
          },
          "required": [
            "session"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "run": {
              "$ref": "#/$defs/RunEvent"
            }
          },
          "required": [
            "run"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "runReconciledOnStartup": {
              "$ref": "#/$defs/RunReconciledOnStartupEvent"
            }
          },
          "required": [
            "runReconciledOnStartup"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "approval": {
              "$ref": "#/$defs/PublicApprovalEvent"
            }
          },
          "required": [
            "approval"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "artifact": {
              "$ref": "#/$defs/ArtifactEvent"
            }
          },
          "required": [
            "artifact"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "contextReceipt": {
              "$ref": "#/$defs/PublicContextReceiptEvent"
            }
          },
          "required": [
            "contextReceipt"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "agentStream": {
              "$ref": "#/$defs/AgentStreamEvent"
            }
          },
          "required": [
            "agentStream"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "tokenUsageRecorded": {
              "$ref": "#/$defs/TokenUsageRecordedEvent"
            }
          },
          "required": [
            "tokenUsageRecorded"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "conflict": {
              "$ref": "#/$defs/ConflictEvent"
            }
          },
          "required": [
            "conflict"
          ],
          "type": "object"
        },
        {
          "additionalProperties": false,
          "properties": {
            "budget": {
              "$ref": "#/$defs/BudgetEvent"
            }
          },
          "required": [
            "budget"
          ],
          "type": "object"
        }
      ]
    },
    "PublicDaemonEventEnvelope": {
      "additionalProperties": false,
      "properties": {
        "daemonInstanceId": {
          "type": "string"
        },
        "event": {
          "$ref": "#/$defs/PublicDaemonEvent"
        },
        "occurredAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "sequence": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "sessionId": {
          "type": "string"
        }
      },
      "required": [
        "daemonInstanceId",
        "sessionId",
        "sequence",
        "occurredAtMs",
        "event"
      ],
      "type": "object"
    },
    "ReceiptKind": {
      "enum": [
        "evidence",
        "patch",
        "testOutput",
        "reviewFinding",
        "artifact",
        "risk",
        "blocker",
        "summary"
      ],
      "type": "string"
    },
    "ReceiptProvenance": {
      "description": "Provenance shape rules:\n- artifact-derived: only `artifact_id` is set; identity = (session, run, kind, artifact_id).\n- event-derived: both `event_seq` and `agent_turn_id` are set; identity = (session, run, kind, event_seq, agent_turn_id).\n- free-form: all identifying fields are None.\n\n`stream_cursor` is descriptive metadata (e.g. for UI navigation) and may be\npresent in any shape. It is never part of the unique identity.",
      "properties": {
        "agentTurnId": {
          "type": [
            "string",
            "null"
          ]
        },
        "artifactId": {
          "type": [
            "string",
            "null"
          ]
        },
        "eventSeq": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "streamCursor": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "type": "object"
    },
    "ReceiptState": {
      "enum": [
        "returned",
        "promoted",
        "quarantined"
      ],
      "type": "string"
    },
    "ReviewFinding": {
      "properties": {
        "file": {
          "type": [
            "string",
            "null"
          ]
        },
        "line": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "message": {
          "type": "string"
        },
        "severity": {
          "$ref": "#/$defs/FindingSeverity"
        },
        "suggestion": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "severity",
        "message"
      ],
      "type": "object"
    },
    "ReviewResult": {
      "properties": {
        "findings": {
          "items": {
            "$ref": "#/$defs/ReviewFinding"
          },
          "type": "array"
        },
        "risks": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "touchedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "verdict": {
          "$ref": "#/$defs/ReviewVerdict"
        }
      },
      "required": [
        "verdict",
        "findings",
        "risks",
        "touchedFiles"
      ],
      "type": "object"
    },
    "ReviewVerdict": {
      "enum": [
        "approve",
        "requestChanges",
        "needsHuman"
      ],
      "type": "string"
    },
    "RunEvent": {
      "properties": {
        "detail": {
          "type": "string"
        },
        "outputContract": {
          "anyOf": [
            {
              "$ref": "#/$defs/OutputContractKind"
            },
            {
              "type": "null"
            }
          ]
        },
        "recipeId": {
          "type": [
            "string",
            "null"
          ]
        },
        "result": {
          "anyOf": [
            {
              "$ref": "#/$defs/CapsuleResult"
            },
            {
              "type": "null"
            }
          ]
        },
        "runId": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/RunStatus"
        }
      },
      "required": [
        "runId",
        "status",
        "detail"
      ],
      "type": "object"
    },
    "RunFailureKind": {
      "enum": [
        "daemonRestartedWhileRunning"
      ],
      "type": "string"
    },
    "RunReconciledOnStartupEvent": {
      "properties": {
        "prevStatus": {
          "$ref": "#/$defs/RunStatus"
        },
        "reason": {
          "$ref": "#/$defs/RunFailureKind"
        },
        "runId": {
          "type": "string"
        }
      },
      "required": [
        "runId",
        "prevStatus",
        "reason"
      ],
      "type": "object"
    },
    "RunStatus": {
      "enum": [
        "queued",
        "running",
        "waitingForApproval",
        "completed",
        "failed",
        "budgetExceeded",
        "cancelled"
      ],
      "type": "string"
    },
    "RunSummary": {
      "properties": {
        "id": {
          "type": "string"
        },
        "objective": {
          "type": "string"
        },
        "runtimeProfileId": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/RunStatus"
        }
      },
      "required": [
        "id",
        "runtimeProfileId",
        "objective",
        "status"
      ],
      "type": "object"
    },
    "RuntimeLanePendingState": {
      "enum": [
        "queued",
        "waitingForApproval",
        "waitingForInput"
      ],
      "type": "string"
    },
    "SessionEvent": {
      "properties": {
        "sessionId": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/SessionStatus"
        }
      },
      "required": [
        "sessionId",
        "status"
      ],
      "type": "object"
    },
    "SessionOverviewLaneStatus": {
      "enum": [
        "idle",
        "active",
        "waitingForApproval",
        "failed",
        "completed",
        "cancelled"
      ],
      "type": "string"
    },
    "SessionStatus": {
      "enum": [
        "idle",
        "running",
        "paused",
        "failed",
        "completed"
      ],
      "type": "string"
    },
    "SessionSummary": {
      "properties": {
        "id": {
          "type": "string"
        },
        "status": {
          "$ref": "#/$defs/SessionStatus"
        },
        "title": {
          "type": "string"
        }
      },
      "required": [
        "id",
        "title",
        "status"
      ],
      "type": "object"
    },
    "TestResult": {
      "properties": {
        "failed": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "failedTestNames": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "logReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "passed": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "skipped": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "total": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        }
      },
      "required": [
        "total",
        "passed",
        "failed",
        "skipped",
        "failedTestNames",
        "logReceiptIds"
      ],
      "type": "object"
    },
    "TokenUsageRecordedEvent": {
      "properties": {
        "cachedTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "capsuleId": {
          "type": [
            "string",
            "null"
          ]
        },
        "completionTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "model": {
          "type": "string"
        },
        "promptTokens": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "provider": {
          "type": "string"
        },
        "reasoningTokens": {
          "anyOf": [
            {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "recordedAtMs": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        "runId": {
          "type": "string"
        }
      },
      "required": [
        "runId",
        "promptTokens",
        "completionTokens",
        "model",
        "provider",
        "recordedAtMs"
      ],
      "type": "object"
    },
    "WorkspaceMode": {
      "enum": [
        "readonly",
        "workspaceWrite",
        "worktreeWrite",
        "repoWriteWithApproval",
        "remoteWorker",
        "containerized",
        "ephemeral"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "approvalAttention": {
      "$ref": "#/$defs/ApprovalAttentionState",
      "description": "Approval attention state owned by the daemon read model."
    },
    "isActive": {
      "description": "True when the session currently owns active or waiting work.",
      "type": "boolean"
    },
    "laneStatus": {
      "$ref": "#/$defs/SessionOverviewLaneStatus",
      "description": "Daemon-owned lane projection for operator-facing session/run state."
    },
    "lastActivityAtMs": {
      "anyOf": [
        {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        },
        {
          "type": "null"
        }
      ],
      "description": "Timestamp of the newest daemon-owned activity item for this session."
    },
    "lastEventPreview": {
      "description": "Compact daemon-owned preview of the newest activity item for this session.",
      "type": [
        "string",
        "null"
      ]
    },
    "latestRun": {
      "anyOf": [
        {
          "$ref": "#/$defs/RunSummary"
        },
        {
          "type": "null"
        }
      ],
      "description": "Most recent run summary for this session, if one exists."
    },
    "pendingApprovalCount": {
      "description": "Count of approvals currently awaiting a decision for this session.",
      "format": "uint32",
      "minimum": 0,
      "type": "integer"
    },
    "recentActivity": {
      "description": "Recent public daemon activity for this session, ordered newest first.",
      "items": {
        "$ref": "#/$defs/PublicDaemonEventEnvelope"
      },
      "type": "array"
    },
    "session": {
      "$ref": "#/$defs/SessionSummary"
    }
  },
  "required": [
    "session",
    "laneStatus",
    "isActive",
    "approvalAttention",
    "pendingApprovalCount"
  ],
  "title": "SessionOverview",
  "type": "object"
},
  SessionStatus: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "idle",
    "running",
    "paused",
    "failed",
    "completed"
  ],
  "title": "SessionStatus",
  "type": "string"
},
  SessionSummary: {
  "$defs": {
    "SessionStatus": {
      "enum": [
        "idle",
        "running",
        "paused",
        "failed",
        "completed"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "id": {
      "type": "string"
    },
    "status": {
      "$ref": "#/$defs/SessionStatus"
    },
    "title": {
      "type": "string"
    }
  },
  "required": [
    "id",
    "title",
    "status"
  ],
  "title": "SessionSummary",
  "type": "object"
},
  StartRunCommand: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "modelId": {
      "type": [
        "string",
        "null"
      ]
    },
    "objective": {
      "type": "string"
    },
    "recipeId": {
      "type": [
        "string",
        "null"
      ]
    },
    "sandboxProfile": {
      "type": [
        "string",
        "null"
      ]
    }
  },
  "required": [
    "objective"
  ],
  "title": "StartRunCommand",
  "type": "object"
},
  DaemonRunCompleteWithResultParams: {
  "$defs": {
    "CapsuleResult": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "debug",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/DebugResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "patch",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/PatchResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "review",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/ReviewResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "test",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/TestResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "plan",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/PlanResult"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "custom",
              "type": "string"
            },
            "value": true
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        }
      ]
    },
    "DebugResult": {
      "properties": {
        "blockers": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "confidence": {
          "maximum": 1,
          "minimum": 0,
          "type": "number"
        },
        "evidenceReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "patchReceiptId": {
          "type": [
            "string",
            "null"
          ]
        },
        "reproduced": {
          "type": "boolean"
        },
        "rootCause": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "reproduced",
        "evidenceReceiptIds",
        "confidence",
        "blockers"
      ],
      "type": "object"
    },
    "FindingSeverity": {
      "enum": [
        "low",
        "medium",
        "high",
        "critical"
      ],
      "type": "string"
    },
    "PatchResult": {
      "properties": {
        "blockers": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "passing": {
          "type": "boolean"
        },
        "patchReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "testsRunReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "touchedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        }
      },
      "required": [
        "patchReceiptIds",
        "touchedFiles",
        "testsRunReceiptIds",
        "passing",
        "blockers"
      ],
      "type": "object"
    },
    "PlanResult": {
      "properties": {
        "estimatedTotalMinutes": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "risks": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "steps": {
          "items": {
            "$ref": "#/$defs/PlanStep"
          },
          "type": "array"
        }
      },
      "required": [
        "steps",
        "risks"
      ],
      "type": "object"
    },
    "PlanStep": {
      "properties": {
        "dependsOn": {
          "items": {
            "format": "uint32",
            "minimum": 0,
            "type": "integer"
          },
          "type": "array"
        },
        "description": {
          "type": [
            "string",
            "null"
          ]
        },
        "estimatedMinutes": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "title": {
          "type": "string"
        }
      },
      "required": [
        "title",
        "dependsOn"
      ],
      "type": "object"
    },
    "ReviewFinding": {
      "properties": {
        "file": {
          "type": [
            "string",
            "null"
          ]
        },
        "line": {
          "format": "uint32",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "message": {
          "type": "string"
        },
        "severity": {
          "$ref": "#/$defs/FindingSeverity"
        },
        "suggestion": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "severity",
        "message"
      ],
      "type": "object"
    },
    "ReviewResult": {
      "properties": {
        "findings": {
          "items": {
            "$ref": "#/$defs/ReviewFinding"
          },
          "type": "array"
        },
        "risks": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "touchedFiles": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "verdict": {
          "$ref": "#/$defs/ReviewVerdict"
        }
      },
      "required": [
        "verdict",
        "findings",
        "risks",
        "touchedFiles"
      ],
      "type": "object"
    },
    "ReviewVerdict": {
      "enum": [
        "approve",
        "requestChanges",
        "needsHuman"
      ],
      "type": "string"
    },
    "TestResult": {
      "properties": {
        "failed": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "failedTestNames": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "logReceiptIds": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "passed": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "skipped": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "total": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        }
      },
      "required": [
        "total",
        "passed",
        "failed",
        "skipped",
        "failedTestNames",
        "logReceiptIds"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "detail": {
      "type": "string"
    },
    "result": {
      "anyOf": [
        {
          "$ref": "#/$defs/CapsuleResult"
        },
        {
          "type": "null"
        }
      ]
    },
    "runId": {
      "type": "string"
    }
  },
  "required": [
    "runId",
    "detail"
  ],
  "title": "DaemonRunCompleteWithResultParams",
  "type": "object"
},
  WorkflowDefinition: {
  "$defs": {
    "WorkflowApprovalPolicy": {
      "additionalProperties": false,
      "properties": {
        "file_write": {
          "$ref": "#/$defs/WorkflowFileWriteApproval"
        },
        "network": {
          "$ref": "#/$defs/WorkflowNetworkApproval"
        },
        "process": {
          "$ref": "#/$defs/WorkflowProcessApproval"
        }
      },
      "required": [
        "file_write",
        "process",
        "network"
      ],
      "type": "object"
    },
    "WorkflowBudgetLimits": {
      "additionalProperties": false,
      "properties": {
        "max_cost_usd": {
          "anyOf": [
            {
              "type": "number"
            },
            {
              "type": "null"
            }
          ]
        },
        "max_tokens": {
          "format": "uint64",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "max_wall_time_ms": {
          "format": "uint64",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        }
      },
      "type": "object"
    },
    "WorkflowBudgets": {
      "additionalProperties": false,
      "properties": {
        "per_capsule": {
          "$ref": "#/$defs/WorkflowBudgetLimits"
        },
        "per_orchestrator": {
          "$ref": "#/$defs/WorkflowBudgetLimits"
        },
        "per_workflow": {
          "$ref": "#/$defs/WorkflowBudgetLimits"
        }
      },
      "required": [
        "per_capsule",
        "per_orchestrator",
        "per_workflow"
      ],
      "type": "object"
    },
    "WorkflowFileWriteApproval": {
      "enum": [
        "ask",
        "auto",
        "deny"
      ],
      "type": "string"
    },
    "WorkflowNetworkApproval": {
      "enum": [
        "allowlist",
        "ask",
        "deny"
      ],
      "type": "string"
    },
    "WorkflowOrchestratorPolicy": {
      "additionalProperties": false,
      "properties": {
        "max_capsules_per_mission": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "max_concurrent_missions": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "retry": {
          "$ref": "#/$defs/WorkflowRetryPolicy"
        }
      },
      "required": [
        "max_concurrent_missions",
        "max_capsules_per_mission",
        "retry"
      ],
      "type": "object"
    },
    "WorkflowOutputRequirement": {
      "enum": [
        "evidence",
        "tests",
        "patch_or_blocker",
        "risk_summary",
        "plan",
        "review_findings"
      ],
      "type": "string"
    },
    "WorkflowOutputsPolicy": {
      "additionalProperties": false,
      "properties": {
        "required": {
          "items": {
            "$ref": "#/$defs/WorkflowOutputRequirement"
          },
          "type": "array"
        }
      },
      "required": [
        "required"
      ],
      "type": "object"
    },
    "WorkflowPolicy": {
      "additionalProperties": false,
      "properties": {
        "approvals": {
          "$ref": "#/$defs/WorkflowApprovalPolicy"
        },
        "network_allowlist": {
          "items": {
            "type": "string"
          },
          "type": "array"
        }
      },
      "required": [
        "approvals"
      ],
      "type": "object"
    },
    "WorkflowProcessApproval": {
      "enum": [
        "ask",
        "ask_for_sensitive",
        "auto",
        "deny"
      ],
      "type": "string"
    },
    "WorkflowRetryPolicy": {
      "additionalProperties": false,
      "properties": {
        "initial_ms": {
          "format": "uint64",
          "minimum": 0,
          "type": "integer"
        },
        "max_ms": {
          "format": "uint64",
          "minimum": 0,
          "type": "integer"
        }
      },
      "required": [
        "initial_ms",
        "max_ms"
      ],
      "type": "object"
    },
    "WorkflowRuntimeProfileRef": {
      "additionalProperties": false,
      "properties": {
        "model": {
          "type": "string"
        },
        "provider": {
          "type": "string"
        },
        "reasoning_effort": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "provider",
        "model"
      ],
      "type": "object"
    },
    "WorkflowSourceBinding": {
      "additionalProperties": false,
      "properties": {
        "active_states": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "kind": {
          "$ref": "#/$defs/WorkflowSourceKind"
        },
        "paths": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "project": {
          "type": [
            "string",
            "null"
          ]
        },
        "repo": {
          "type": [
            "string",
            "null"
          ]
        },
        "terminal_states": {
          "items": {
            "type": "string"
          },
          "type": "array"
        }
      },
      "required": [
        "kind",
        "active_states",
        "terminal_states"
      ],
      "type": "object"
    },
    "WorkflowSourceKind": {
      "enum": [
        "linear",
        "github_issues",
        "github_pr_reviews",
        "local_tasks",
        "mission_board",
        "cli"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "additionalProperties": false,
  "properties": {
    "budgets": {
      "$ref": "#/$defs/WorkflowBudgets"
    },
    "kind": {
      "type": "string"
    },
    "name": {
      "type": "string"
    },
    "orchestrator": {
      "$ref": "#/$defs/WorkflowOrchestratorPolicy"
    },
    "outputs": {
      "$ref": "#/$defs/WorkflowOutputsPolicy"
    },
    "policy": {
      "$ref": "#/$defs/WorkflowPolicy"
    },
    "runtime_profiles": {
      "additionalProperties": {
        "$ref": "#/$defs/WorkflowRuntimeProfileRef"
      },
      "type": "object"
    },
    "source": {
      "$ref": "#/$defs/WorkflowSourceBinding"
    }
  },
  "required": [
    "kind",
    "name",
    "source",
    "orchestrator",
    "policy",
    "runtime_profiles",
    "outputs",
    "budgets"
  ],
  "title": "WorkflowDefinition",
  "type": "object"
},
  WorkflowSourceBinding: {
  "$defs": {
    "WorkflowSourceKind": {
      "enum": [
        "linear",
        "github_issues",
        "github_pr_reviews",
        "local_tasks",
        "mission_board",
        "cli"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "additionalProperties": false,
  "properties": {
    "active_states": {
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "kind": {
      "$ref": "#/$defs/WorkflowSourceKind"
    },
    "paths": {
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "project": {
      "type": [
        "string",
        "null"
      ]
    },
    "repo": {
      "type": [
        "string",
        "null"
      ]
    },
    "terminal_states": {
      "items": {
        "type": "string"
      },
      "type": "array"
    }
  },
  "required": [
    "kind",
    "active_states",
    "terminal_states"
  ],
  "title": "WorkflowSourceBinding",
  "type": "object"
},
  WorkflowSourceKind: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "linear",
    "github_issues",
    "github_pr_reviews",
    "local_tasks",
    "mission_board",
    "cli"
  ],
  "title": "WorkflowSourceKind",
  "type": "string"
},
  WorkflowOrchestratorPolicy: {
  "$defs": {
    "WorkflowRetryPolicy": {
      "additionalProperties": false,
      "properties": {
        "initial_ms": {
          "format": "uint64",
          "minimum": 0,
          "type": "integer"
        },
        "max_ms": {
          "format": "uint64",
          "minimum": 0,
          "type": "integer"
        }
      },
      "required": [
        "initial_ms",
        "max_ms"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "additionalProperties": false,
  "properties": {
    "max_capsules_per_mission": {
      "format": "uint32",
      "minimum": 0,
      "type": "integer"
    },
    "max_concurrent_missions": {
      "format": "uint32",
      "minimum": 0,
      "type": "integer"
    },
    "retry": {
      "$ref": "#/$defs/WorkflowRetryPolicy"
    }
  },
  "required": [
    "max_concurrent_missions",
    "max_capsules_per_mission",
    "retry"
  ],
  "title": "WorkflowOrchestratorPolicy",
  "type": "object"
},
  WorkflowRetryPolicy: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "additionalProperties": false,
  "properties": {
    "initial_ms": {
      "format": "uint64",
      "minimum": 0,
      "type": "integer"
    },
    "max_ms": {
      "format": "uint64",
      "minimum": 0,
      "type": "integer"
    }
  },
  "required": [
    "initial_ms",
    "max_ms"
  ],
  "title": "WorkflowRetryPolicy",
  "type": "object"
},
  WorkflowPolicy: {
  "$defs": {
    "WorkflowApprovalPolicy": {
      "additionalProperties": false,
      "properties": {
        "file_write": {
          "$ref": "#/$defs/WorkflowFileWriteApproval"
        },
        "network": {
          "$ref": "#/$defs/WorkflowNetworkApproval"
        },
        "process": {
          "$ref": "#/$defs/WorkflowProcessApproval"
        }
      },
      "required": [
        "file_write",
        "process",
        "network"
      ],
      "type": "object"
    },
    "WorkflowFileWriteApproval": {
      "enum": [
        "ask",
        "auto",
        "deny"
      ],
      "type": "string"
    },
    "WorkflowNetworkApproval": {
      "enum": [
        "allowlist",
        "ask",
        "deny"
      ],
      "type": "string"
    },
    "WorkflowProcessApproval": {
      "enum": [
        "ask",
        "ask_for_sensitive",
        "auto",
        "deny"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "additionalProperties": false,
  "properties": {
    "approvals": {
      "$ref": "#/$defs/WorkflowApprovalPolicy"
    },
    "network_allowlist": {
      "items": {
        "type": "string"
      },
      "type": "array"
    }
  },
  "required": [
    "approvals"
  ],
  "title": "WorkflowPolicy",
  "type": "object"
},
  WorkflowApprovalPolicy: {
  "$defs": {
    "WorkflowFileWriteApproval": {
      "enum": [
        "ask",
        "auto",
        "deny"
      ],
      "type": "string"
    },
    "WorkflowNetworkApproval": {
      "enum": [
        "allowlist",
        "ask",
        "deny"
      ],
      "type": "string"
    },
    "WorkflowProcessApproval": {
      "enum": [
        "ask",
        "ask_for_sensitive",
        "auto",
        "deny"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "additionalProperties": false,
  "properties": {
    "file_write": {
      "$ref": "#/$defs/WorkflowFileWriteApproval"
    },
    "network": {
      "$ref": "#/$defs/WorkflowNetworkApproval"
    },
    "process": {
      "$ref": "#/$defs/WorkflowProcessApproval"
    }
  },
  "required": [
    "file_write",
    "process",
    "network"
  ],
  "title": "WorkflowApprovalPolicy",
  "type": "object"
},
  WorkflowFileWriteApproval: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "ask",
    "auto",
    "deny"
  ],
  "title": "WorkflowFileWriteApproval",
  "type": "string"
},
  WorkflowProcessApproval: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "ask",
    "ask_for_sensitive",
    "auto",
    "deny"
  ],
  "title": "WorkflowProcessApproval",
  "type": "string"
},
  WorkflowNetworkApproval: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "allowlist",
    "ask",
    "deny"
  ],
  "title": "WorkflowNetworkApproval",
  "type": "string"
},
  WorkflowRuntimeProfileRef: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "additionalProperties": false,
  "properties": {
    "model": {
      "type": "string"
    },
    "provider": {
      "type": "string"
    },
    "reasoning_effort": {
      "type": [
        "string",
        "null"
      ]
    }
  },
  "required": [
    "provider",
    "model"
  ],
  "title": "WorkflowRuntimeProfileRef",
  "type": "object"
},
  WorkflowOutputsPolicy: {
  "$defs": {
    "WorkflowOutputRequirement": {
      "enum": [
        "evidence",
        "tests",
        "patch_or_blocker",
        "risk_summary",
        "plan",
        "review_findings"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "additionalProperties": false,
  "properties": {
    "required": {
      "items": {
        "$ref": "#/$defs/WorkflowOutputRequirement"
      },
      "type": "array"
    }
  },
  "required": [
    "required"
  ],
  "title": "WorkflowOutputsPolicy",
  "type": "object"
},
  WorkflowOutputRequirement: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "evidence",
    "tests",
    "patch_or_blocker",
    "risk_summary",
    "plan",
    "review_findings"
  ],
  "title": "WorkflowOutputRequirement",
  "type": "string"
},
  WorkflowBudgets: {
  "$defs": {
    "WorkflowBudgetLimits": {
      "additionalProperties": false,
      "properties": {
        "max_cost_usd": {
          "anyOf": [
            {
              "type": "number"
            },
            {
              "type": "null"
            }
          ]
        },
        "max_tokens": {
          "format": "uint64",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "max_wall_time_ms": {
          "format": "uint64",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        }
      },
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "additionalProperties": false,
  "properties": {
    "per_capsule": {
      "$ref": "#/$defs/WorkflowBudgetLimits"
    },
    "per_orchestrator": {
      "$ref": "#/$defs/WorkflowBudgetLimits"
    },
    "per_workflow": {
      "$ref": "#/$defs/WorkflowBudgetLimits"
    }
  },
  "required": [
    "per_capsule",
    "per_orchestrator",
    "per_workflow"
  ],
  "title": "WorkflowBudgets",
  "type": "object"
},
  WorkflowBudgetLimits: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "additionalProperties": false,
  "properties": {
    "max_cost_usd": {
      "anyOf": [
        {
          "type": "number"
        },
        {
          "type": "null"
        }
      ]
    },
    "max_tokens": {
      "format": "uint64",
      "minimum": 0,
      "type": [
        "integer",
        "null"
      ]
    },
    "max_wall_time_ms": {
      "format": "uint64",
      "minimum": 0,
      "type": [
        "integer",
        "null"
      ]
    }
  },
  "title": "WorkflowBudgetLimits",
  "type": "object"
},
  WorkflowLoadParams: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "additionalProperties": false,
  "properties": {
    "path": {
      "type": "string"
    }
  },
  "required": [
    "path"
  ],
  "title": "WorkflowLoadParams",
  "type": "object"
},
  WorkflowReloadParams: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "additionalProperties": false,
  "title": "WorkflowReloadParams",
  "type": "object"
},
  WorkflowValidateParams: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "additionalProperties": false,
  "properties": {
    "contents": {
      "type": [
        "string",
        "null"
      ]
    },
    "path": {
      "type": [
        "string",
        "null"
      ]
    }
  },
  "title": "WorkflowValidateParams",
  "type": "object"
},
  WorkflowValidationReport: {
  "$defs": {
    "WorkflowValidationError": {
      "properties": {
        "message": {
          "type": "string"
        },
        "path": {
          "type": "string"
        }
      },
      "required": [
        "path",
        "message"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "errors": {
      "items": {
        "$ref": "#/$defs/WorkflowValidationError"
      },
      "type": "array"
    },
    "valid": {
      "type": "boolean"
    }
  },
  "required": [
    "valid",
    "errors"
  ],
  "title": "WorkflowValidationReport",
  "type": "object"
},
  WorkflowValidationError: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "message": {
      "type": "string"
    },
    "path": {
      "type": "string"
    }
  },
  "required": [
    "path",
    "message"
  ],
  "title": "WorkflowValidationError",
  "type": "object"
},
  WorkflowStatusResult: {
  "$defs": {
    "WorkflowLoadedStatus": {
      "properties": {
        "name": {
          "type": "string"
        },
        "path": {
          "type": "string"
        },
        "runtimeProfileCount": {
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
        "sourceKind": {
          "$ref": "#/$defs/WorkflowSourceKind"
        },
        "version": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "name",
        "path",
        "sourceKind",
        "runtimeProfileCount",
        "version"
      ],
      "type": "object"
    },
    "WorkflowReloadOutcome": {
      "oneOf": [
        {
          "properties": {
            "name": {
              "type": "string"
            },
            "prev_name": {
              "type": [
                "string",
                "null"
              ]
            },
            "status": {
              "const": "reloaded",
              "type": "string"
            },
            "version": {
              "maxLength": 20,
              "pattern": "^[0-9]+$",
              "type": "string"
            }
          },
          "required": [
            "status",
            "name",
            "version"
          ],
          "type": "object"
        },
        {
          "properties": {
            "errors": {
              "items": {
                "$ref": "#/$defs/WorkflowValidationError"
              },
              "type": "array"
            },
            "status": {
              "const": "failed",
              "type": "string"
            }
          },
          "required": [
            "status",
            "errors"
          ],
          "type": "object"
        }
      ]
    },
    "WorkflowSourceKind": {
      "enum": [
        "linear",
        "github_issues",
        "github_pr_reviews",
        "local_tasks",
        "mission_board",
        "cli"
      ],
      "type": "string"
    },
    "WorkflowValidationError": {
      "properties": {
        "message": {
          "type": "string"
        },
        "path": {
          "type": "string"
        }
      },
      "required": [
        "path",
        "message"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "lastReload": {
      "anyOf": [
        {
          "$ref": "#/$defs/WorkflowReloadOutcome"
        },
        {
          "type": "null"
        }
      ]
    },
    "loaded": {
      "anyOf": [
        {
          "$ref": "#/$defs/WorkflowLoadedStatus"
        },
        {
          "type": "null"
        }
      ]
    }
  },
  "title": "WorkflowStatusResult",
  "type": "object"
},
  WorkflowLoadedStatus: {
  "$defs": {
    "WorkflowSourceKind": {
      "enum": [
        "linear",
        "github_issues",
        "github_pr_reviews",
        "local_tasks",
        "mission_board",
        "cli"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "name": {
      "type": "string"
    },
    "path": {
      "type": "string"
    },
    "runtimeProfileCount": {
      "format": "uint32",
      "minimum": 0,
      "type": "integer"
    },
    "sourceKind": {
      "$ref": "#/$defs/WorkflowSourceKind"
    },
    "version": {
      "maxLength": 20,
      "pattern": "^[0-9]+$",
      "type": "string"
    }
  },
  "required": [
    "name",
    "path",
    "sourceKind",
    "runtimeProfileCount",
    "version"
  ],
  "title": "WorkflowLoadedStatus",
  "type": "object"
},
  WorkflowReloadOutcome: {
  "$defs": {
    "WorkflowValidationError": {
      "properties": {
        "message": {
          "type": "string"
        },
        "path": {
          "type": "string"
        }
      },
      "required": [
        "path",
        "message"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "oneOf": [
    {
      "properties": {
        "name": {
          "type": "string"
        },
        "prev_name": {
          "type": [
            "string",
            "null"
          ]
        },
        "status": {
          "const": "reloaded",
          "type": "string"
        },
        "version": {
          "maxLength": 20,
          "pattern": "^[0-9]+$",
          "type": "string"
        }
      },
      "required": [
        "status",
        "name",
        "version"
      ],
      "type": "object"
    },
    {
      "properties": {
        "errors": {
          "items": {
            "$ref": "#/$defs/WorkflowValidationError"
          },
          "type": "array"
        },
        "status": {
          "const": "failed",
          "type": "string"
        }
      },
      "required": [
        "status",
        "errors"
      ],
      "type": "object"
    }
  ],
  "title": "WorkflowReloadOutcome"
},
  AgentRuntimeStrategyId: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "string",
  "type": "string"
},
  AgentRuntimeModelId: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "string",
  "type": "string"
},
  AgentRuntimeModelRef: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "contextLimit": {
      "format": "uint64",
      "minimum": 0,
      "type": [
        "integer",
        "null"
      ]
    },
    "displayName": {
      "type": "string"
    },
    "id": {
      "type": "string"
    },
    "inputTokenCostMicros": {
      "format": "uint64",
      "minimum": 0,
      "type": [
        "integer",
        "null"
      ]
    },
    "outputTokenCostMicros": {
      "format": "uint64",
      "minimum": 0,
      "type": [
        "integer",
        "null"
      ]
    }
  },
  "required": [
    "id",
    "displayName"
  ],
  "title": "AgentRuntimeModelRef",
  "type": "object"
},
  AgentRuntimeStrategyHealthStatus: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "unknown",
    "ready",
    "degraded",
    "unavailable"
  ],
  "title": "AgentRuntimeStrategyHealthStatus",
  "type": "string"
},
  AgentRuntimeStrategyHealth: {
  "$defs": {
    "AgentRuntimeStrategyHealthStatus": {
      "enum": [
        "unknown",
        "ready",
        "degraded",
        "unavailable"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "message": {
      "type": [
        "string",
        "null"
      ]
    },
    "status": {
      "$ref": "#/$defs/AgentRuntimeStrategyHealthStatus"
    }
  },
  "required": [
    "status"
  ],
  "title": "AgentRuntimeStrategyHealth",
  "type": "object"
},
  AgentRuntimeStrategyInfo: {
  "$defs": {
    "AgentRuntimeModelAvailability": {
      "enum": [
        "enumerated",
        "currentOnly",
        "unsupported",
        "unavailable",
        "unknown"
      ],
      "type": "string"
    },
    "AgentRuntimeModelCapability": {
      "properties": {
        "availability": {
          "$ref": "#/$defs/AgentRuntimeModelAvailability"
        },
        "canSetModel": {
          "type": "boolean"
        },
        "currentModelId": {
          "type": [
            "string",
            "null"
          ]
        },
        "detail": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "availability",
        "canSetModel"
      ],
      "type": "object"
    },
    "AgentRuntimeModelRef": {
      "properties": {
        "contextLimit": {
          "format": "uint64",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "displayName": {
          "type": "string"
        },
        "id": {
          "type": "string"
        },
        "inputTokenCostMicros": {
          "format": "uint64",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "outputTokenCostMicros": {
          "format": "uint64",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "displayName"
      ],
      "type": "object"
    },
    "AgentRuntimeStrategyHealth": {
      "properties": {
        "message": {
          "type": [
            "string",
            "null"
          ]
        },
        "status": {
          "$ref": "#/$defs/AgentRuntimeStrategyHealthStatus"
        }
      },
      "required": [
        "status"
      ],
      "type": "object"
    },
    "AgentRuntimeStrategyHealthStatus": {
      "enum": [
        "unknown",
        "ready",
        "degraded",
        "unavailable"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "displayName": {
      "type": "string"
    },
    "health": {
      "$ref": "#/$defs/AgentRuntimeStrategyHealth"
    },
    "id": {
      "type": "string"
    },
    "modelCapability": {
      "$ref": "#/$defs/AgentRuntimeModelCapability"
    },
    "models": {
      "items": {
        "$ref": "#/$defs/AgentRuntimeModelRef"
      },
      "type": "array"
    }
  },
  "required": [
    "id",
    "displayName",
    "modelCapability",
    "health"
  ],
  "title": "AgentRuntimeStrategyInfo",
  "type": "object"
},
  AuthProfileId: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "string",
  "type": "string"
},
  AuthProfileConnectionState: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "loggedOut",
    "pendingLogin",
    "connected",
    "error"
  ],
  "title": "AuthProfileConnectionState",
  "type": "string"
},
  AuthProfileLoginMethod: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "browser",
    "deviceCode",
    "manual"
  ],
  "title": "AuthProfileLoginMethod",
  "type": "string"
},
  AuthProfileRef: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "displayName": {
      "type": "string"
    },
    "id": {
      "type": "string"
    },
    "providerId": {
      "type": "string"
    }
  },
  "required": [
    "id",
    "providerId",
    "displayName"
  ],
  "title": "AuthProfileRef",
  "type": "object"
},
  AuthProfileState: {
  "$defs": {
    "AuthProfileActionHint": {
      "properties": {
        "command": {
          "type": [
            "string",
            "null"
          ]
        },
        "description": {
          "type": [
            "string",
            "null"
          ]
        },
        "label": {
          "type": "string"
        }
      },
      "required": [
        "label"
      ],
      "type": "object"
    },
    "AuthProfileConnectionState": {
      "enum": [
        "loggedOut",
        "pendingLogin",
        "connected",
        "error"
      ],
      "type": "string"
    },
    "AuthProfileManagementMode": {
      "enum": [
        "interactive",
        "nativeAcpAuth",
        "terminalCliDelegated",
        "environment",
        "none",
        "unknown"
      ],
      "type": "string"
    },
    "AuthProfileMethodInfo": {
      "properties": {
        "displayName": {
          "type": "string"
        },
        "id": {
          "type": "string"
        },
        "managementMode": {
          "$ref": "#/$defs/AuthProfileManagementMode"
        }
      },
      "required": [
        "id",
        "displayName",
        "managementMode"
      ],
      "type": "object"
    },
    "AuthProfileRef": {
      "properties": {
        "displayName": {
          "type": "string"
        },
        "id": {
          "type": "string"
        },
        "providerId": {
          "type": "string"
        }
      },
      "required": [
        "id",
        "providerId",
        "displayName"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "action": {
      "anyOf": [
        {
          "$ref": "#/$defs/AuthProfileActionHint"
        },
        {
          "type": "null"
        }
      ]
    },
    "canLogin": {
      "type": "boolean"
    },
    "canLogout": {
      "type": "boolean"
    },
    "connectionState": {
      "$ref": "#/$defs/AuthProfileConnectionState"
    },
    "lastError": {
      "type": [
        "string",
        "null"
      ]
    },
    "managementMode": {
      "$ref": "#/$defs/AuthProfileManagementMode"
    },
    "methods": {
      "items": {
        "$ref": "#/$defs/AuthProfileMethodInfo"
      },
      "type": "array"
    },
    "platformOrgLinked": {
      "type": [
        "boolean",
        "null"
      ]
    },
    "profile": {
      "$ref": "#/$defs/AuthProfileRef"
    },
    "setupSteps": {
      "items": {
        "type": "string"
      },
      "type": "array"
    }
  },
  "required": [
    "profile",
    "connectionState",
    "managementMode",
    "canLogin",
    "canLogout"
  ],
  "title": "AuthProfileState",
  "type": "object"
},
  AuthProfileLoginChallenge: {
  "$defs": {
    "AuthProfileLoginMethod": {
      "enum": [
        "browser",
        "deviceCode",
        "manual"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "authProfileId": {
      "type": "string"
    },
    "authorizeUrl": {
      "type": [
        "string",
        "null"
      ]
    },
    "manualBrowserUrl": {
      "type": [
        "string",
        "null"
      ]
    },
    "method": {
      "$ref": "#/$defs/AuthProfileLoginMethod"
    },
    "userCode": {
      "type": [
        "string",
        "null"
      ]
    }
  },
  "required": [
    "authProfileId",
    "method"
  ],
  "title": "AuthProfileLoginChallenge",
  "type": "object"
},
  AuthProfileLoginResult: {
  "$defs": {
    "AuthProfileActionHint": {
      "properties": {
        "command": {
          "type": [
            "string",
            "null"
          ]
        },
        "description": {
          "type": [
            "string",
            "null"
          ]
        },
        "label": {
          "type": "string"
        }
      },
      "required": [
        "label"
      ],
      "type": "object"
    },
    "AuthProfileConnectionState": {
      "enum": [
        "loggedOut",
        "pendingLogin",
        "connected",
        "error"
      ],
      "type": "string"
    },
    "AuthProfileLoginChallenge": {
      "properties": {
        "authProfileId": {
          "type": "string"
        },
        "authorizeUrl": {
          "type": [
            "string",
            "null"
          ]
        },
        "manualBrowserUrl": {
          "type": [
            "string",
            "null"
          ]
        },
        "method": {
          "$ref": "#/$defs/AuthProfileLoginMethod"
        },
        "userCode": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "authProfileId",
        "method"
      ],
      "type": "object"
    },
    "AuthProfileLoginMethod": {
      "enum": [
        "browser",
        "deviceCode",
        "manual"
      ],
      "type": "string"
    },
    "AuthProfileManagementMode": {
      "enum": [
        "interactive",
        "nativeAcpAuth",
        "terminalCliDelegated",
        "environment",
        "none",
        "unknown"
      ],
      "type": "string"
    },
    "AuthProfileMethodInfo": {
      "properties": {
        "displayName": {
          "type": "string"
        },
        "id": {
          "type": "string"
        },
        "managementMode": {
          "$ref": "#/$defs/AuthProfileManagementMode"
        }
      },
      "required": [
        "id",
        "displayName",
        "managementMode"
      ],
      "type": "object"
    },
    "AuthProfileRef": {
      "properties": {
        "displayName": {
          "type": "string"
        },
        "id": {
          "type": "string"
        },
        "providerId": {
          "type": "string"
        }
      },
      "required": [
        "id",
        "providerId",
        "displayName"
      ],
      "type": "object"
    },
    "AuthProfileState": {
      "properties": {
        "action": {
          "anyOf": [
            {
              "$ref": "#/$defs/AuthProfileActionHint"
            },
            {
              "type": "null"
            }
          ]
        },
        "canLogin": {
          "type": "boolean"
        },
        "canLogout": {
          "type": "boolean"
        },
        "connectionState": {
          "$ref": "#/$defs/AuthProfileConnectionState"
        },
        "lastError": {
          "type": [
            "string",
            "null"
          ]
        },
        "managementMode": {
          "$ref": "#/$defs/AuthProfileManagementMode"
        },
        "methods": {
          "items": {
            "$ref": "#/$defs/AuthProfileMethodInfo"
          },
          "type": "array"
        },
        "platformOrgLinked": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "profile": {
          "$ref": "#/$defs/AuthProfileRef"
        },
        "setupSteps": {
          "items": {
            "type": "string"
          },
          "type": "array"
        }
      },
      "required": [
        "profile",
        "connectionState",
        "managementMode",
        "canLogin",
        "canLogout"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "authProfile": {
      "$ref": "#/$defs/AuthProfileState"
    },
    "challenge": {
      "anyOf": [
        {
          "$ref": "#/$defs/AuthProfileLoginChallenge"
        },
        {
          "type": "null"
        }
      ]
    }
  },
  "required": [
    "authProfile"
  ],
  "title": "AuthProfileLoginResult",
  "type": "object"
},
  AuthProfileLogoutResult: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "authProfileId": {
      "type": "string"
    },
    "disconnected": {
      "type": "boolean"
    }
  },
  "required": [
    "authProfileId",
    "disconnected"
  ],
  "title": "AuthProfileLogoutResult",
  "type": "object"
},
  RuntimeExtensionId: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "string",
  "type": "string"
},
  RuntimeExtensionDescriptor: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "description": {
      "type": "string"
    },
    "displayName": {
      "type": "string"
    },
    "id": {
      "type": "string"
    }
  },
  "required": [
    "id",
    "displayName",
    "description"
  ],
  "title": "RuntimeExtensionDescriptor",
  "type": "object"
},
  RuntimeExtensionAvailability: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "available",
    "unavailable"
  ],
  "title": "RuntimeExtensionAvailability",
  "type": "string"
},
  RuntimeExtensionMcpServer: {
  "$defs": {
    "RuntimeExtensionEnvVar": {
      "properties": {
        "name": {
          "type": "string"
        },
        "value": {
          "type": "string"
        }
      },
      "required": [
        "name",
        "value"
      ],
      "type": "object"
    },
    "RuntimeExtensionHttpHeader": {
      "properties": {
        "name": {
          "type": "string"
        },
        "value": {
          "type": "string"
        }
      },
      "required": [
        "name",
        "value"
      ],
      "type": "object"
    },
    "RuntimeExtensionMcpHttpServer": {
      "properties": {
        "headers": {
          "items": {
            "$ref": "#/$defs/RuntimeExtensionHttpHeader"
          },
          "type": "array"
        },
        "name": {
          "type": "string"
        },
        "url": {
          "type": "string"
        }
      },
      "required": [
        "name",
        "url"
      ],
      "type": "object"
    },
    "RuntimeExtensionMcpStdioServer": {
      "properties": {
        "args": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "command": {
          "type": "string"
        },
        "env": {
          "items": {
            "$ref": "#/$defs/RuntimeExtensionEnvVar"
          },
          "type": "array"
        },
        "name": {
          "type": "string"
        }
      },
      "required": [
        "name",
        "command"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "oneOf": [
    {
      "$ref": "#/$defs/RuntimeExtensionMcpStdioServer",
      "properties": {
        "transport": {
          "const": "stdio",
          "type": "string"
        }
      },
      "required": [
        "transport"
      ],
      "type": "object"
    },
    {
      "$ref": "#/$defs/RuntimeExtensionMcpHttpServer",
      "properties": {
        "transport": {
          "const": "http",
          "type": "string"
        }
      },
      "required": [
        "transport"
      ],
      "type": "object"
    }
  ],
  "title": "RuntimeExtensionMcpServer"
},
  RuntimeExtensionMcpStdioServer: {
  "$defs": {
    "RuntimeExtensionEnvVar": {
      "properties": {
        "name": {
          "type": "string"
        },
        "value": {
          "type": "string"
        }
      },
      "required": [
        "name",
        "value"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "args": {
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "command": {
      "type": "string"
    },
    "env": {
      "items": {
        "$ref": "#/$defs/RuntimeExtensionEnvVar"
      },
      "type": "array"
    },
    "name": {
      "type": "string"
    }
  },
  "required": [
    "name",
    "command"
  ],
  "title": "RuntimeExtensionMcpStdioServer",
  "type": "object"
},
  RuntimeExtensionMcpHttpServer: {
  "$defs": {
    "RuntimeExtensionHttpHeader": {
      "properties": {
        "name": {
          "type": "string"
        },
        "value": {
          "type": "string"
        }
      },
      "required": [
        "name",
        "value"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "headers": {
      "items": {
        "$ref": "#/$defs/RuntimeExtensionHttpHeader"
      },
      "type": "array"
    },
    "name": {
      "type": "string"
    },
    "url": {
      "type": "string"
    }
  },
  "required": [
    "name",
    "url"
  ],
  "title": "RuntimeExtensionMcpHttpServer",
  "type": "object"
},
  RuntimeExtensionEnvVar: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "name": {
      "type": "string"
    },
    "value": {
      "type": "string"
    }
  },
  "required": [
    "name",
    "value"
  ],
  "title": "RuntimeExtensionEnvVar",
  "type": "object"
},
  RuntimeExtensionHttpHeader: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "name": {
      "type": "string"
    },
    "value": {
      "type": "string"
    }
  },
  "required": [
    "name",
    "value"
  ],
  "title": "RuntimeExtensionHttpHeader",
  "type": "object"
},
  RuntimeExtensionState: {
  "$defs": {
    "RuntimeExtensionAvailability": {
      "enum": [
        "available",
        "unavailable"
      ],
      "type": "string"
    },
    "RuntimeExtensionDescriptor": {
      "properties": {
        "description": {
          "type": "string"
        },
        "displayName": {
          "type": "string"
        },
        "id": {
          "type": "string"
        }
      },
      "required": [
        "id",
        "displayName",
        "description"
      ],
      "type": "object"
    },
    "RuntimeExtensionEnvVar": {
      "properties": {
        "name": {
          "type": "string"
        },
        "value": {
          "type": "string"
        }
      },
      "required": [
        "name",
        "value"
      ],
      "type": "object"
    },
    "RuntimeExtensionHttpHeader": {
      "properties": {
        "name": {
          "type": "string"
        },
        "value": {
          "type": "string"
        }
      },
      "required": [
        "name",
        "value"
      ],
      "type": "object"
    },
    "RuntimeExtensionMcpHttpServer": {
      "properties": {
        "headers": {
          "items": {
            "$ref": "#/$defs/RuntimeExtensionHttpHeader"
          },
          "type": "array"
        },
        "name": {
          "type": "string"
        },
        "url": {
          "type": "string"
        }
      },
      "required": [
        "name",
        "url"
      ],
      "type": "object"
    },
    "RuntimeExtensionMcpServer": {
      "oneOf": [
        {
          "$ref": "#/$defs/RuntimeExtensionMcpStdioServer",
          "properties": {
            "transport": {
              "const": "stdio",
              "type": "string"
            }
          },
          "required": [
            "transport"
          ],
          "type": "object"
        },
        {
          "$ref": "#/$defs/RuntimeExtensionMcpHttpServer",
          "properties": {
            "transport": {
              "const": "http",
              "type": "string"
            }
          },
          "required": [
            "transport"
          ],
          "type": "object"
        }
      ]
    },
    "RuntimeExtensionMcpStdioServer": {
      "properties": {
        "args": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "command": {
          "type": "string"
        },
        "env": {
          "items": {
            "$ref": "#/$defs/RuntimeExtensionEnvVar"
          },
          "type": "array"
        },
        "name": {
          "type": "string"
        }
      },
      "required": [
        "name",
        "command"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "availability": {
      "$ref": "#/$defs/RuntimeExtensionAvailability"
    },
    "descriptor": {
      "$ref": "#/$defs/RuntimeExtensionDescriptor"
    },
    "enabled": {
      "type": "boolean"
    },
    "mcpServer": {
      "anyOf": [
        {
          "$ref": "#/$defs/RuntimeExtensionMcpServer"
        },
        {
          "type": "null"
        }
      ]
    }
  },
  "required": [
    "descriptor",
    "availability",
    "enabled"
  ],
  "title": "RuntimeExtensionState",
  "type": "object"
},
  RuntimeProfileId: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "string",
  "type": "string"
},
  RuntimePolicyMode: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "requireApproval",
    "allow",
    "deny"
  ],
  "title": "RuntimePolicyMode",
  "type": "string"
},
  LocalModelApiStandard: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "openAiChatCompletions",
    "ollamaOpenAi",
    "lmStudioOpenAi",
    "llamaCppOpenAi",
    "vllmOpenAi",
    "tgiMessages"
  ],
  "title": "LocalModelApiStandard",
  "type": "string"
},
  LocalModelAuthMode: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "none",
    "bearerEnv"
  ],
  "title": "LocalModelAuthMode",
  "type": "string"
},
  LocalModelEndpointCapabilities: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "parallelToolCalls": {
      "type": [
        "boolean",
        "null"
      ]
    },
    "responsesApi": {
      "type": [
        "boolean",
        "null"
      ]
    },
    "streaming": {
      "type": [
        "boolean",
        "null"
      ]
    },
    "tools": {
      "type": [
        "boolean",
        "null"
      ]
    },
    "vision": {
      "type": [
        "boolean",
        "null"
      ]
    }
  },
  "title": "LocalModelEndpointCapabilities",
  "type": "object"
},
  LocalModelEndpointConfig: {
  "$defs": {
    "LocalModelApiStandard": {
      "enum": [
        "openAiChatCompletions",
        "ollamaOpenAi",
        "lmStudioOpenAi",
        "llamaCppOpenAi",
        "vllmOpenAi",
        "tgiMessages"
      ],
      "type": "string"
    },
    "LocalModelAuthMode": {
      "enum": [
        "none",
        "bearerEnv"
      ],
      "type": "string"
    },
    "LocalModelEndpointCapabilities": {
      "properties": {
        "parallelToolCalls": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "responsesApi": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "streaming": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "tools": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "vision": {
          "type": [
            "boolean",
            "null"
          ]
        }
      },
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "apiKeyEnv": {
      "type": [
        "string",
        "null"
      ]
    },
    "apiStandard": {
      "$ref": "#/$defs/LocalModelApiStandard"
    },
    "authMode": {
      "$ref": "#/$defs/LocalModelAuthMode"
    },
    "baseUrl": {
      "type": "string"
    },
    "capabilities": {
      "anyOf": [
        {
          "$ref": "#/$defs/LocalModelEndpointCapabilities"
        },
        {
          "type": "null"
        }
      ]
    },
    "defaultModel": {
      "type": [
        "string",
        "null"
      ]
    },
    "modelDiscovery": {
      "default": false,
      "type": "boolean"
    }
  },
  "required": [
    "baseUrl",
    "apiStandard",
    "authMode"
  ],
  "title": "LocalModelEndpointConfig",
  "type": "object"
},
  RuntimeProfileSummary: {
  "$defs": {
    "LocalModelApiStandard": {
      "enum": [
        "openAiChatCompletions",
        "ollamaOpenAi",
        "lmStudioOpenAi",
        "llamaCppOpenAi",
        "vllmOpenAi",
        "tgiMessages"
      ],
      "type": "string"
    },
    "LocalModelAuthMode": {
      "enum": [
        "none",
        "bearerEnv"
      ],
      "type": "string"
    },
    "LocalModelEndpointCapabilities": {
      "properties": {
        "parallelToolCalls": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "responsesApi": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "streaming": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "tools": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "vision": {
          "type": [
            "boolean",
            "null"
          ]
        }
      },
      "type": "object"
    },
    "LocalModelEndpointConfig": {
      "properties": {
        "apiKeyEnv": {
          "type": [
            "string",
            "null"
          ]
        },
        "apiStandard": {
          "$ref": "#/$defs/LocalModelApiStandard"
        },
        "authMode": {
          "$ref": "#/$defs/LocalModelAuthMode"
        },
        "baseUrl": {
          "type": "string"
        },
        "capabilities": {
          "anyOf": [
            {
              "$ref": "#/$defs/LocalModelEndpointCapabilities"
            },
            {
              "type": "null"
            }
          ]
        },
        "defaultModel": {
          "type": [
            "string",
            "null"
          ]
        },
        "modelDiscovery": {
          "default": false,
          "type": "boolean"
        }
      },
      "required": [
        "baseUrl",
        "apiStandard",
        "authMode"
      ],
      "type": "object"
    },
    "RuntimePolicyMode": {
      "enum": [
        "requireApproval",
        "allow",
        "deny"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "authProfileId": {
      "type": [
        "string",
        "null"
      ]
    },
    "displayName": {
      "type": "string"
    },
    "id": {
      "type": "string"
    },
    "localEndpoint": {
      "anyOf": [
        {
          "$ref": "#/$defs/LocalModelEndpointConfig"
        },
        {
          "type": "null"
        }
      ]
    },
    "modelId": {
      "type": [
        "string",
        "null"
      ]
    },
    "policyMode": {
      "$ref": "#/$defs/RuntimePolicyMode"
    },
    "providerId": {
      "type": "string"
    }
  },
  "required": [
    "id",
    "displayName",
    "providerId",
    "policyMode"
  ],
  "title": "RuntimeProfileSummary",
  "type": "object"
},
  RuntimeProfileModelIdPatch: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "oneOf": [
    {
      "properties": {
        "kind": {
          "const": "set",
          "type": "string"
        },
        "value": {
          "type": "string"
        }
      },
      "required": [
        "kind",
        "value"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "clear",
          "type": "string"
        }
      },
      "required": [
        "kind"
      ],
      "type": "object"
    }
  ],
  "title": "RuntimeProfileModelIdPatch"
},
  RuntimeProfileAuthProfilePatch: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "oneOf": [
    {
      "properties": {
        "kind": {
          "const": "set",
          "type": "string"
        },
        "value": {
          "type": "string"
        }
      },
      "required": [
        "kind",
        "value"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "clear",
          "type": "string"
        }
      },
      "required": [
        "kind"
      ],
      "type": "object"
    }
  ],
  "title": "RuntimeProfileAuthProfilePatch"
},
  RuntimeProfileLocalEndpointPatch: {
  "$defs": {
    "LocalModelApiStandard": {
      "enum": [
        "openAiChatCompletions",
        "ollamaOpenAi",
        "lmStudioOpenAi",
        "llamaCppOpenAi",
        "vllmOpenAi",
        "tgiMessages"
      ],
      "type": "string"
    },
    "LocalModelAuthMode": {
      "enum": [
        "none",
        "bearerEnv"
      ],
      "type": "string"
    },
    "LocalModelEndpointCapabilities": {
      "properties": {
        "parallelToolCalls": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "responsesApi": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "streaming": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "tools": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "vision": {
          "type": [
            "boolean",
            "null"
          ]
        }
      },
      "type": "object"
    },
    "LocalModelEndpointConfig": {
      "properties": {
        "apiKeyEnv": {
          "type": [
            "string",
            "null"
          ]
        },
        "apiStandard": {
          "$ref": "#/$defs/LocalModelApiStandard"
        },
        "authMode": {
          "$ref": "#/$defs/LocalModelAuthMode"
        },
        "baseUrl": {
          "type": "string"
        },
        "capabilities": {
          "anyOf": [
            {
              "$ref": "#/$defs/LocalModelEndpointCapabilities"
            },
            {
              "type": "null"
            }
          ]
        },
        "defaultModel": {
          "type": [
            "string",
            "null"
          ]
        },
        "modelDiscovery": {
          "default": false,
          "type": "boolean"
        }
      },
      "required": [
        "baseUrl",
        "apiStandard",
        "authMode"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "oneOf": [
    {
      "properties": {
        "kind": {
          "const": "set",
          "type": "string"
        },
        "value": {
          "$ref": "#/$defs/LocalModelEndpointConfig"
        }
      },
      "required": [
        "kind",
        "value"
      ],
      "type": "object"
    },
    {
      "properties": {
        "kind": {
          "const": "clear",
          "type": "string"
        }
      },
      "required": [
        "kind"
      ],
      "type": "object"
    }
  ],
  "title": "RuntimeProfileLocalEndpointPatch"
},
  RuntimeProfilePatch: {
  "$defs": {
    "LocalModelApiStandard": {
      "enum": [
        "openAiChatCompletions",
        "ollamaOpenAi",
        "lmStudioOpenAi",
        "llamaCppOpenAi",
        "vllmOpenAi",
        "tgiMessages"
      ],
      "type": "string"
    },
    "LocalModelAuthMode": {
      "enum": [
        "none",
        "bearerEnv"
      ],
      "type": "string"
    },
    "LocalModelEndpointCapabilities": {
      "properties": {
        "parallelToolCalls": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "responsesApi": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "streaming": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "tools": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "vision": {
          "type": [
            "boolean",
            "null"
          ]
        }
      },
      "type": "object"
    },
    "LocalModelEndpointConfig": {
      "properties": {
        "apiKeyEnv": {
          "type": [
            "string",
            "null"
          ]
        },
        "apiStandard": {
          "$ref": "#/$defs/LocalModelApiStandard"
        },
        "authMode": {
          "$ref": "#/$defs/LocalModelAuthMode"
        },
        "baseUrl": {
          "type": "string"
        },
        "capabilities": {
          "anyOf": [
            {
              "$ref": "#/$defs/LocalModelEndpointCapabilities"
            },
            {
              "type": "null"
            }
          ]
        },
        "defaultModel": {
          "type": [
            "string",
            "null"
          ]
        },
        "modelDiscovery": {
          "default": false,
          "type": "boolean"
        }
      },
      "required": [
        "baseUrl",
        "apiStandard",
        "authMode"
      ],
      "type": "object"
    },
    "RuntimePolicyMode": {
      "enum": [
        "requireApproval",
        "allow",
        "deny"
      ],
      "type": "string"
    },
    "RuntimeProfileAuthProfilePatch": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "set",
              "type": "string"
            },
            "value": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "clear",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "RuntimeProfileLocalEndpointPatch": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "set",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/LocalModelEndpointConfig"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "clear",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "RuntimeProfileModelIdPatch": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "set",
              "type": "string"
            },
            "value": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "clear",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "authProfile": {
      "anyOf": [
        {
          "$ref": "#/$defs/RuntimeProfileAuthProfilePatch"
        },
        {
          "type": "null"
        }
      ]
    },
    "displayName": {
      "type": [
        "string",
        "null"
      ]
    },
    "localEndpoint": {
      "anyOf": [
        {
          "$ref": "#/$defs/RuntimeProfileLocalEndpointPatch"
        },
        {
          "type": "null"
        }
      ]
    },
    "modelId": {
      "anyOf": [
        {
          "$ref": "#/$defs/RuntimeProfileModelIdPatch"
        },
        {
          "type": "null"
        }
      ]
    },
    "policyMode": {
      "anyOf": [
        {
          "$ref": "#/$defs/RuntimePolicyMode"
        },
        {
          "type": "null"
        }
      ]
    },
    "providerId": {
      "type": [
        "string",
        "null"
      ]
    }
  },
  "title": "RuntimeProfilePatch",
  "type": "object"
},
  AgentRuntimeSelection: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "runtimeProfileId": {
      "type": "string"
    }
  },
  "required": [
    "runtimeProfileId"
  ],
  "title": "AgentRuntimeSelection",
  "type": "object"
},
  AgentRuntimeSnapshot: {
  "$defs": {
    "AgentRuntimeModelAvailability": {
      "enum": [
        "enumerated",
        "currentOnly",
        "unsupported",
        "unavailable",
        "unknown"
      ],
      "type": "string"
    },
    "AgentRuntimeModelCapability": {
      "properties": {
        "availability": {
          "$ref": "#/$defs/AgentRuntimeModelAvailability"
        },
        "canSetModel": {
          "type": "boolean"
        },
        "currentModelId": {
          "type": [
            "string",
            "null"
          ]
        },
        "detail": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "availability",
        "canSetModel"
      ],
      "type": "object"
    },
    "AgentRuntimeModelRef": {
      "properties": {
        "contextLimit": {
          "format": "uint64",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "displayName": {
          "type": "string"
        },
        "id": {
          "type": "string"
        },
        "inputTokenCostMicros": {
          "format": "uint64",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "outputTokenCostMicros": {
          "format": "uint64",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "displayName"
      ],
      "type": "object"
    },
    "AgentRuntimeSelection": {
      "properties": {
        "runtimeProfileId": {
          "type": "string"
        }
      },
      "required": [
        "runtimeProfileId"
      ],
      "type": "object"
    },
    "AgentRuntimeStrategyHealth": {
      "properties": {
        "message": {
          "type": [
            "string",
            "null"
          ]
        },
        "status": {
          "$ref": "#/$defs/AgentRuntimeStrategyHealthStatus"
        }
      },
      "required": [
        "status"
      ],
      "type": "object"
    },
    "AgentRuntimeStrategyHealthStatus": {
      "enum": [
        "unknown",
        "ready",
        "degraded",
        "unavailable"
      ],
      "type": "string"
    },
    "AgentRuntimeStrategyInfo": {
      "properties": {
        "displayName": {
          "type": "string"
        },
        "health": {
          "$ref": "#/$defs/AgentRuntimeStrategyHealth"
        },
        "id": {
          "type": "string"
        },
        "modelCapability": {
          "$ref": "#/$defs/AgentRuntimeModelCapability"
        },
        "models": {
          "items": {
            "$ref": "#/$defs/AgentRuntimeModelRef"
          },
          "type": "array"
        }
      },
      "required": [
        "id",
        "displayName",
        "modelCapability",
        "health"
      ],
      "type": "object"
    },
    "AuthProfileActionHint": {
      "properties": {
        "command": {
          "type": [
            "string",
            "null"
          ]
        },
        "description": {
          "type": [
            "string",
            "null"
          ]
        },
        "label": {
          "type": "string"
        }
      },
      "required": [
        "label"
      ],
      "type": "object"
    },
    "AuthProfileConnectionState": {
      "enum": [
        "loggedOut",
        "pendingLogin",
        "connected",
        "error"
      ],
      "type": "string"
    },
    "AuthProfileManagementMode": {
      "enum": [
        "interactive",
        "nativeAcpAuth",
        "terminalCliDelegated",
        "environment",
        "none",
        "unknown"
      ],
      "type": "string"
    },
    "AuthProfileMethodInfo": {
      "properties": {
        "displayName": {
          "type": "string"
        },
        "id": {
          "type": "string"
        },
        "managementMode": {
          "$ref": "#/$defs/AuthProfileManagementMode"
        }
      },
      "required": [
        "id",
        "displayName",
        "managementMode"
      ],
      "type": "object"
    },
    "AuthProfileRef": {
      "properties": {
        "displayName": {
          "type": "string"
        },
        "id": {
          "type": "string"
        },
        "providerId": {
          "type": "string"
        }
      },
      "required": [
        "id",
        "providerId",
        "displayName"
      ],
      "type": "object"
    },
    "AuthProfileState": {
      "properties": {
        "action": {
          "anyOf": [
            {
              "$ref": "#/$defs/AuthProfileActionHint"
            },
            {
              "type": "null"
            }
          ]
        },
        "canLogin": {
          "type": "boolean"
        },
        "canLogout": {
          "type": "boolean"
        },
        "connectionState": {
          "$ref": "#/$defs/AuthProfileConnectionState"
        },
        "lastError": {
          "type": [
            "string",
            "null"
          ]
        },
        "managementMode": {
          "$ref": "#/$defs/AuthProfileManagementMode"
        },
        "methods": {
          "items": {
            "$ref": "#/$defs/AuthProfileMethodInfo"
          },
          "type": "array"
        },
        "platformOrgLinked": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "profile": {
          "$ref": "#/$defs/AuthProfileRef"
        },
        "setupSteps": {
          "items": {
            "type": "string"
          },
          "type": "array"
        }
      },
      "required": [
        "profile",
        "connectionState",
        "managementMode",
        "canLogin",
        "canLogout"
      ],
      "type": "object"
    },
    "LocalModelApiStandard": {
      "enum": [
        "openAiChatCompletions",
        "ollamaOpenAi",
        "lmStudioOpenAi",
        "llamaCppOpenAi",
        "vllmOpenAi",
        "tgiMessages"
      ],
      "type": "string"
    },
    "LocalModelAuthMode": {
      "enum": [
        "none",
        "bearerEnv"
      ],
      "type": "string"
    },
    "LocalModelEndpointCapabilities": {
      "properties": {
        "parallelToolCalls": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "responsesApi": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "streaming": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "tools": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "vision": {
          "type": [
            "boolean",
            "null"
          ]
        }
      },
      "type": "object"
    },
    "LocalModelEndpointConfig": {
      "properties": {
        "apiKeyEnv": {
          "type": [
            "string",
            "null"
          ]
        },
        "apiStandard": {
          "$ref": "#/$defs/LocalModelApiStandard"
        },
        "authMode": {
          "$ref": "#/$defs/LocalModelAuthMode"
        },
        "baseUrl": {
          "type": "string"
        },
        "capabilities": {
          "anyOf": [
            {
              "$ref": "#/$defs/LocalModelEndpointCapabilities"
            },
            {
              "type": "null"
            }
          ]
        },
        "defaultModel": {
          "type": [
            "string",
            "null"
          ]
        },
        "modelDiscovery": {
          "default": false,
          "type": "boolean"
        }
      },
      "required": [
        "baseUrl",
        "apiStandard",
        "authMode"
      ],
      "type": "object"
    },
    "RuntimeExtensionAvailability": {
      "enum": [
        "available",
        "unavailable"
      ],
      "type": "string"
    },
    "RuntimeExtensionDescriptor": {
      "properties": {
        "description": {
          "type": "string"
        },
        "displayName": {
          "type": "string"
        },
        "id": {
          "type": "string"
        }
      },
      "required": [
        "id",
        "displayName",
        "description"
      ],
      "type": "object"
    },
    "RuntimeExtensionEnvVar": {
      "properties": {
        "name": {
          "type": "string"
        },
        "value": {
          "type": "string"
        }
      },
      "required": [
        "name",
        "value"
      ],
      "type": "object"
    },
    "RuntimeExtensionHttpHeader": {
      "properties": {
        "name": {
          "type": "string"
        },
        "value": {
          "type": "string"
        }
      },
      "required": [
        "name",
        "value"
      ],
      "type": "object"
    },
    "RuntimeExtensionMcpHttpServer": {
      "properties": {
        "headers": {
          "items": {
            "$ref": "#/$defs/RuntimeExtensionHttpHeader"
          },
          "type": "array"
        },
        "name": {
          "type": "string"
        },
        "url": {
          "type": "string"
        }
      },
      "required": [
        "name",
        "url"
      ],
      "type": "object"
    },
    "RuntimeExtensionMcpServer": {
      "oneOf": [
        {
          "$ref": "#/$defs/RuntimeExtensionMcpStdioServer",
          "properties": {
            "transport": {
              "const": "stdio",
              "type": "string"
            }
          },
          "required": [
            "transport"
          ],
          "type": "object"
        },
        {
          "$ref": "#/$defs/RuntimeExtensionMcpHttpServer",
          "properties": {
            "transport": {
              "const": "http",
              "type": "string"
            }
          },
          "required": [
            "transport"
          ],
          "type": "object"
        }
      ]
    },
    "RuntimeExtensionMcpStdioServer": {
      "properties": {
        "args": {
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "command": {
          "type": "string"
        },
        "env": {
          "items": {
            "$ref": "#/$defs/RuntimeExtensionEnvVar"
          },
          "type": "array"
        },
        "name": {
          "type": "string"
        }
      },
      "required": [
        "name",
        "command"
      ],
      "type": "object"
    },
    "RuntimeExtensionState": {
      "properties": {
        "availability": {
          "$ref": "#/$defs/RuntimeExtensionAvailability"
        },
        "descriptor": {
          "$ref": "#/$defs/RuntimeExtensionDescriptor"
        },
        "enabled": {
          "type": "boolean"
        },
        "mcpServer": {
          "anyOf": [
            {
              "$ref": "#/$defs/RuntimeExtensionMcpServer"
            },
            {
              "type": "null"
            }
          ]
        }
      },
      "required": [
        "descriptor",
        "availability",
        "enabled"
      ],
      "type": "object"
    },
    "RuntimePolicyMode": {
      "enum": [
        "requireApproval",
        "allow",
        "deny"
      ],
      "type": "string"
    },
    "RuntimeProfileSummary": {
      "properties": {
        "authProfileId": {
          "type": [
            "string",
            "null"
          ]
        },
        "displayName": {
          "type": "string"
        },
        "id": {
          "type": "string"
        },
        "localEndpoint": {
          "anyOf": [
            {
              "$ref": "#/$defs/LocalModelEndpointConfig"
            },
            {
              "type": "null"
            }
          ]
        },
        "modelId": {
          "type": [
            "string",
            "null"
          ]
        },
        "policyMode": {
          "$ref": "#/$defs/RuntimePolicyMode"
        },
        "providerId": {
          "type": "string"
        }
      },
      "required": [
        "id",
        "displayName",
        "providerId",
        "policyMode"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "authProfiles": {
      "items": {
        "$ref": "#/$defs/AuthProfileState"
      },
      "type": "array"
    },
    "providers": {
      "items": {
        "$ref": "#/$defs/AgentRuntimeStrategyInfo"
      },
      "type": "array"
    },
    "runtimeExtensions": {
      "items": {
        "$ref": "#/$defs/RuntimeExtensionState"
      },
      "type": "array"
    },
    "runtimeProfiles": {
      "items": {
        "$ref": "#/$defs/RuntimeProfileSummary"
      },
      "type": "array"
    },
    "selection": {
      "$ref": "#/$defs/AgentRuntimeSelection"
    }
  },
  "required": [
    "selection"
  ],
  "title": "AgentRuntimeSnapshot",
  "type": "object"
},
  GetAgentRuntimeQuery: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "GetAgentRuntimeQuery",
  "type": "object"
},
  DaemonAgentRuntimeSelectProfileParams: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "runtimeProfileId": {
      "type": "string"
    }
  },
  "required": [
    "runtimeProfileId"
  ],
  "title": "DaemonAgentRuntimeSelectProfileParams",
  "type": "object"
},
  DaemonAgentRuntimePatchProfileParams: {
  "$defs": {
    "LocalModelApiStandard": {
      "enum": [
        "openAiChatCompletions",
        "ollamaOpenAi",
        "lmStudioOpenAi",
        "llamaCppOpenAi",
        "vllmOpenAi",
        "tgiMessages"
      ],
      "type": "string"
    },
    "LocalModelAuthMode": {
      "enum": [
        "none",
        "bearerEnv"
      ],
      "type": "string"
    },
    "LocalModelEndpointCapabilities": {
      "properties": {
        "parallelToolCalls": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "responsesApi": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "streaming": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "tools": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "vision": {
          "type": [
            "boolean",
            "null"
          ]
        }
      },
      "type": "object"
    },
    "LocalModelEndpointConfig": {
      "properties": {
        "apiKeyEnv": {
          "type": [
            "string",
            "null"
          ]
        },
        "apiStandard": {
          "$ref": "#/$defs/LocalModelApiStandard"
        },
        "authMode": {
          "$ref": "#/$defs/LocalModelAuthMode"
        },
        "baseUrl": {
          "type": "string"
        },
        "capabilities": {
          "anyOf": [
            {
              "$ref": "#/$defs/LocalModelEndpointCapabilities"
            },
            {
              "type": "null"
            }
          ]
        },
        "defaultModel": {
          "type": [
            "string",
            "null"
          ]
        },
        "modelDiscovery": {
          "default": false,
          "type": "boolean"
        }
      },
      "required": [
        "baseUrl",
        "apiStandard",
        "authMode"
      ],
      "type": "object"
    },
    "RuntimePolicyMode": {
      "enum": [
        "requireApproval",
        "allow",
        "deny"
      ],
      "type": "string"
    },
    "RuntimeProfileAuthProfilePatch": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "set",
              "type": "string"
            },
            "value": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "clear",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "RuntimeProfileLocalEndpointPatch": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "set",
              "type": "string"
            },
            "value": {
              "$ref": "#/$defs/LocalModelEndpointConfig"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "clear",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "RuntimeProfileModelIdPatch": {
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "set",
              "type": "string"
            },
            "value": {
              "type": "string"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "type": "object"
        },
        {
          "properties": {
            "kind": {
              "const": "clear",
              "type": "string"
            }
          },
          "required": [
            "kind"
          ],
          "type": "object"
        }
      ]
    },
    "RuntimeProfilePatch": {
      "properties": {
        "authProfile": {
          "anyOf": [
            {
              "$ref": "#/$defs/RuntimeProfileAuthProfilePatch"
            },
            {
              "type": "null"
            }
          ]
        },
        "displayName": {
          "type": [
            "string",
            "null"
          ]
        },
        "localEndpoint": {
          "anyOf": [
            {
              "$ref": "#/$defs/RuntimeProfileLocalEndpointPatch"
            },
            {
              "type": "null"
            }
          ]
        },
        "modelId": {
          "anyOf": [
            {
              "$ref": "#/$defs/RuntimeProfileModelIdPatch"
            },
            {
              "type": "null"
            }
          ]
        },
        "policyMode": {
          "anyOf": [
            {
              "$ref": "#/$defs/RuntimePolicyMode"
            },
            {
              "type": "null"
            }
          ]
        },
        "providerId": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "patch": {
      "$ref": "#/$defs/RuntimeProfilePatch"
    },
    "runtimeProfileId": {
      "type": "string"
    }
  },
  "required": [
    "runtimeProfileId",
    "patch"
  ],
  "title": "DaemonAgentRuntimePatchProfileParams",
  "type": "object"
},
  DaemonAgentRuntimeAuthLoginParams: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "authProfileId": {
      "type": "string"
    }
  },
  "required": [
    "authProfileId"
  ],
  "title": "DaemonAgentRuntimeAuthLoginParams",
  "type": "object"
},
  DaemonAgentRuntimeAuthLogoutParams: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "authProfileId": {
      "type": "string"
    }
  },
  "required": [
    "authProfileId"
  ],
  "title": "DaemonAgentRuntimeAuthLogoutParams",
  "type": "object"
},
  DaemonAgentRuntimeSetExtensionEnabledParams: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "enabled": {
      "type": "boolean"
    },
    "extensionId": {
      "type": "string"
    }
  },
  "required": [
    "extensionId",
    "enabled"
  ],
  "title": "DaemonAgentRuntimeSetExtensionEnabledParams",
  "type": "object"
},
  DaemonAgentRuntimeTestLocalEndpointParams: {
  "$defs": {
    "LocalModelApiStandard": {
      "enum": [
        "openAiChatCompletions",
        "ollamaOpenAi",
        "lmStudioOpenAi",
        "llamaCppOpenAi",
        "vllmOpenAi",
        "tgiMessages"
      ],
      "type": "string"
    },
    "LocalModelAuthMode": {
      "enum": [
        "none",
        "bearerEnv"
      ],
      "type": "string"
    },
    "LocalModelEndpointCapabilities": {
      "properties": {
        "parallelToolCalls": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "responsesApi": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "streaming": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "tools": {
          "type": [
            "boolean",
            "null"
          ]
        },
        "vision": {
          "type": [
            "boolean",
            "null"
          ]
        }
      },
      "type": "object"
    },
    "LocalModelEndpointConfig": {
      "properties": {
        "apiKeyEnv": {
          "type": [
            "string",
            "null"
          ]
        },
        "apiStandard": {
          "$ref": "#/$defs/LocalModelApiStandard"
        },
        "authMode": {
          "$ref": "#/$defs/LocalModelAuthMode"
        },
        "baseUrl": {
          "type": "string"
        },
        "capabilities": {
          "anyOf": [
            {
              "$ref": "#/$defs/LocalModelEndpointCapabilities"
            },
            {
              "type": "null"
            }
          ]
        },
        "defaultModel": {
          "type": [
            "string",
            "null"
          ]
        },
        "modelDiscovery": {
          "default": false,
          "type": "boolean"
        }
      },
      "required": [
        "baseUrl",
        "apiStandard",
        "authMode"
      ],
      "type": "object"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "endpoint": {
      "$ref": "#/$defs/LocalModelEndpointConfig"
    },
    "modelId": {
      "type": [
        "string",
        "null"
      ]
    },
    "testToolCall": {
      "default": false,
      "type": "boolean"
    }
  },
  "required": [
    "endpoint"
  ],
  "title": "DaemonAgentRuntimeTestLocalEndpointParams",
  "type": "object"
},
  LocalModelEndpointTestStatus: {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "enum": [
    "ready",
    "degraded",
    "toolsUnsupported",
    "unreachable",
    "invalidConfig"
  ],
  "title": "LocalModelEndpointTestStatus",
  "type": "string"
},
  LocalModelEndpointTestResult: {
  "$defs": {
    "AgentRuntimeModelRef": {
      "properties": {
        "contextLimit": {
          "format": "uint64",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "displayName": {
          "type": "string"
        },
        "id": {
          "type": "string"
        },
        "inputTokenCostMicros": {
          "format": "uint64",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        },
        "outputTokenCostMicros": {
          "format": "uint64",
          "minimum": 0,
          "type": [
            "integer",
            "null"
          ]
        }
      },
      "required": [
        "id",
        "displayName"
      ],
      "type": "object"
    },
    "LocalModelEndpointTestStatus": {
      "enum": [
        "ready",
        "degraded",
        "toolsUnsupported",
        "unreachable",
        "invalidConfig"
      ],
      "type": "string"
    }
  },
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "properties": {
    "message": {
      "type": [
        "string",
        "null"
      ]
    },
    "models": {
      "default": [],
      "items": {
        "$ref": "#/$defs/AgentRuntimeModelRef"
      },
      "type": "array"
    },
    "status": {
      "$ref": "#/$defs/LocalModelEndpointTestStatus"
    },
    "toolsSupported": {
      "type": [
        "boolean",
        "null"
      ]
    }
  },
  "required": [
    "status"
  ],
  "title": "LocalModelEndpointTestResult",
  "type": "object"
},
} as const;
