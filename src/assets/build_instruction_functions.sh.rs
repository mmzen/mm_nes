#!/usr/bin/bash

OUTPUT_FILE="instruction_functions.txt"

echo > $OUTPUT_FILE

cat ../instructions_macro.rs |
    awk -F, '{ print $NF }' |
    sed '/{{/d' |
    sed '/}}/d' |
    sed 's/);$//' |
    sed 's/^[ \t]*//' |
    sort |
    uniq |
    sed '/^[ ^t]*$/d' | while read line ; do
    func_name=`echo $line`

    echo "=> $func_name" 1>&2
    echo -e "\tfn $func_name(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
\t\tErr(CpuError::UnImplemented(format!(\"{:?}\", self.opcode)))
\t}
" >>  $OUTPUT_FILE

done
