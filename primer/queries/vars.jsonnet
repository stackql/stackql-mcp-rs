// config for finops.iql
// external var 'month' (YYYY-MM) supplied with --var, e.g. --var month=2026-08
local month = std.extVar('month');
local year = std.parseInt(std.split(month, '-')[0]);
local mon = std.parseInt(std.split(month, '-')[1]);
// Cost Explorer's End is exclusive, so the window runs to the first of next month
local next = if mon == 12 then '%d-01-01' % (year + 1) else '%d-%02d-01' % [year, mon + 1];

{
  period: {
    start: month + '-01',
    end: next,
  },
  granularity: 'MONTHLY',
}
