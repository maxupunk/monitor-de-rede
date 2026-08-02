export interface AlertRuleCondition {
  field: string
  operator: 'eq' | 'neq' | 'gt' | 'gte' | 'lt' | 'lte' | 'contains'
  value: unknown
}

export class RuleEvaluator {
  evaluate(condition: AlertRuleCondition, targetData: Record<string, unknown>): boolean {
    const fieldValue = targetData[condition.field]
    if (fieldValue === undefined || fieldValue === null) {
      return false
    }

    switch (condition.operator) {
      case 'eq':
        return fieldValue === condition.value
      case 'neq':
        return fieldValue !== condition.value
      case 'gt':
        return Number(fieldValue) > Number(condition.value)
      case 'gte':
        return Number(fieldValue) >= Number(condition.value)
      case 'lt':
        return Number(fieldValue) < Number(condition.value)
      case 'lte':
        return Number(fieldValue) <= Number(condition.value)
      case 'contains':
        return String(fieldValue).includes(String(condition.value))
      default:
        return false
    }
  }
}
