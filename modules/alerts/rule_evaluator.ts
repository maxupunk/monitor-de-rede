export interface AlertRuleCondition {
  field: string
  operator: 'eq' | 'neq' | 'gt' | 'gte' | 'lt' | 'lte'
  value: unknown
}

export class RuleEvaluator {
  evaluate(condition: AlertRuleCondition, currentValue: unknown): boolean {
    switch (condition.operator) {
      case 'eq': return currentValue === condition.value
      case 'neq': return currentValue !== condition.value
      case 'gt': return (currentValue as number) > (condition.value as number)
      case 'gte': return (currentValue as number) >= (condition.value as number)
      case 'lt': return (currentValue as number) < (condition.value as number)
      case 'lte': return (currentValue as number) <= (condition.value as number)
      default: return false
    }
  }
}
