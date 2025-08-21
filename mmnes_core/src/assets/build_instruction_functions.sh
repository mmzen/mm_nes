#!/usr/bin/bash

OUTPUT_FILE="instruction_functions.txt"

echo > $OUTPUT_FILE

cat ../instructions_macro_all.rs |
    grep Illegal |
    awk -F, '{ print $(NF - 1) }' |
    sed '/{{/d' |
    sed '/}}/d' |
    sed 's/);$//' |
    sed 's/^[ \t]*//' |
    sort |
    uniq |
    sed '/^[ ^t]*$/d' | while read line ; do
    func_name=`echo $line`

    echo "=> $func_name" 1>&2
    echo -e "\tfn $func_name(&self, _: &mut Cpu6502, _: &Operand) -> Result<(), CpuError> {
\t\tErr(CpuError::Unimplemented(format!(\"{:?}\", self.opcode)))
\t}
" >>  $OUTPUT_FILE

done
