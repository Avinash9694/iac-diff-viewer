import cfnBefore from './cfn-before.yaml?raw';
import cfnAfter from './cfn-after.yaml?raw';
import tfPlan from './tf-plan.json?raw';
import tfStateBefore from './tf-state-before.json?raw';
import tfStateAfter from './tf-state-after.json?raw';

export interface Sample {
  id: string;
  name: string;
  description: string;
  before: string;
  after: string;
}

export const SAMPLES: readonly Sample[] = [
  {
    id: 'cfn-web-tier',
    name: 'CloudFormation — RDS removed, SSH opened',
    description:
      'YAML templates with short-form intrinsics. The database leaves the stack, SSH is opened to the world, an IAM policy gains s3:*, and a DynamoDB table loses its Retain policy.',
    before: cfnBefore,
    after: cfnAfter,
  },
  {
    id: 'tf-plan',
    name: 'Terraform plan — resize, tags, bucket made public',
    description:
      'A single `terraform show -json` plan: prior state is diffed against planned values. An instance resizes, tags change, a public-access block is switched off and a queue loses its KMS key.',
    before: tfPlan,
    after: '',
  },
  {
    id: 'tf-rename',
    name: 'Terraform state — renamed addresses',
    description:
      'Two state snapshots where three resources changed address. Identical properties are matched as renames rather than a destroy plus a create.',
    before: tfStateBefore,
    after: tfStateAfter,
  },
];

export const DEFAULT_SAMPLE_ID = SAMPLES[0]?.id ?? '';

export function findSample(id: string): Sample | undefined {
  return SAMPLES.find((s) => s.id === id);
}
