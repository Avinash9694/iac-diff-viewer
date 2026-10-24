import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { App } from '../App';

describe('App', () => {
  it('loads the default sample, shows the summary and lets the user inspect a resource', async () => {
    const user = userEvent.setup();
    render(<App />);

    const summary = await screen.findByRole('region', { name: 'Diff summary' });
    expect(within(summary).getByText('Overall risk').parentElement).toHaveTextContent('Critical');
    expect(within(summary).getByText(/CloudFormation \(YAML\) · 11 resources/)).toBeInTheDocument();

    const list = screen.getByRole('listbox', { name: /Changed resources/ });
    const options = within(list).getAllByRole('option');
    expect(options.length).toBe(9);
    expect(options[0]).toHaveAttribute('aria-selected', 'true');
    expect(options[0]).toHaveTextContent('Database');

    // The first (highest-risk) row is selected and detailed by default.
    const detail = screen.getByRole('region', { name: 'Detail for Database' });
    expect(within(detail).getByText(/Stateful resource removed/)).toBeInTheDocument();

    await user.click(within(list).getByRole('option', { name: /WebSecurityGroup/ }));
    const sgDetail = screen.getByRole('region', { name: 'Detail for WebSecurityGroup' });
    expect(within(sgDetail).getByText(/Ingress opened to the world/)).toBeInTheDocument();
    expect(
      within(sgDetail).getByRole('row', { name: /SecurityGroupIngress\[1\]\.CidrIp/ }),
    ).toBeInTheDocument();

    await user.click(within(sgDetail).getByRole('tab', { name: 'Raw JSON diff' }));
    expect(
      within(sgDetail).getByText(/Unified diff of the normalised property JSON/),
    ).toBeInTheDocument();
  });

  it('filters the list and navigates with the keyboard', async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole('region', { name: 'Diff summary' });

    await user.type(screen.getByRole('searchbox', { name: 'Filter resources' }), 'orders');
    const list = screen.getByRole('listbox', { name: /Changed resources/ });
    expect(within(list).getAllByRole('option')).toHaveLength(1);
    expect(screen.getByRole('region', { name: 'Detail for OrdersTable' })).toBeInTheDocument();

    await user.clear(screen.getByRole('searchbox', { name: 'Filter resources' }));
    const first = within(list).getAllByRole('option')[0];
    if (!first) throw new Error('no rows');
    first.focus();
    await user.keyboard('{ArrowDown}');
    expect(screen.getByRole('region', { name: 'Detail for WebSecurityGroup' })).toBeInTheDocument();
    await user.keyboard('{End}');
    expect(screen.getByRole('region', { name: 'Detail for ApiFunction' })).toBeInTheDocument();
  });

  it('switches samples and reports a Terraform plan diffed against itself', async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole('region', { name: 'Diff summary' });

    await user.selectOptions(screen.getByRole('combobox', { name: 'Load a sample' }), 'tf-plan');
    const summary = await screen.findByText(/Terraform plan — planned values/);
    expect(summary).toBeInTheDocument();
    expect(
      within(screen.getByRole('region', { name: 'Diff summary' })).getByText('Overall risk')
        .parentElement,
    ).toHaveTextContent('High');
  });

  it('surfaces a structured parse error from the engine', async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole('region', { name: 'Diff summary' });

    await user.click(screen.getByRole('button', { name: 'Edit documents' }));
    const after = screen.getByRole('textbox', { name: 'After' });
    await user.clear(after);
    await user.type(after, '{{ "Resources": ');
    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent(/After document: Invalid JSON/);
    expect(alert).toHaveTextContent(/line \d+, column \d+/);
  });
});
